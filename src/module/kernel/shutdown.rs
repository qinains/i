use super::binding::BindingLease;
use super::context::Graph;
use super::{Context, EffectScope, Fiber, Task};
use anyhow::{Result, anyhow};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Notify;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShutdownReport {
    pub closed: bool,
    pub errors: Vec<String>,
    pub retained: Vec<String>,
}

#[derive(Default)]
pub(crate) struct Drain {
    result: Mutex<Option<ShutdownReport>>,
    changed: Notify,
}
impl Drain {
    fn finish(&self, report: ShutdownReport) {
        self.result.lock().unwrap().get_or_insert(report);
        self.changed.notify_waiters();
    }
    async fn wait(&self) -> ShutdownReport {
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if let Some(report) = self.result.lock().unwrap().clone() {
                return report;
            }
            changed.await;
        }
    }
}

impl Context {
    pub fn request_shutdown(&self) {
        drop(self.shutdown());
    }
    pub fn shutdown(&self) -> Task<ShutdownReport> {
        self.shutdown_with_timeout(Duration::from_secs(10))
    }
    pub fn shutdown_with_timeout(&self, timeout: Duration) -> Task<ShutdownReport> {
        let drain = begin(&self.graph, true);
        let observer = drain.clone();
        let graph = self.graph.clone();
        tokio::spawn(async move {
            if tokio::time::timeout(timeout, observer.wait()).await.is_err() {
                let table = graph.table.lock().unwrap();
                let mut retained: Vec<_> = table.fibers.keys().map(|id| format!("fiber:{}", id.0)).collect();
                retained.push("root effects".into());
                retained.sort();
                observer.finish(ShutdownReport {
                    closed: false,
                    errors: vec!["shutdown timed out; in-use resources were retained".into()],
                    retained,
                });
            }
        });
        Box::pin(async move { drain.wait().await })
    }
    pub fn restart(&self) -> Task<Result<()>> {
        if self.graph.table.lock().unwrap().terminal {
            return Box::pin(async { Err(anyhow!("application is terminally closed")) });
        }
        let drain = begin(&self.graph, false);
        let graph = self.graph.clone();
        Box::pin(async move {
            let report = drain.wait().await;
            anyhow::ensure!(!graph.table.lock().unwrap().terminal, "application is terminally closed");
            anyhow::ensure!(report.closed, "root restart did not drain");
            Ok(())
        })
    }
}

fn begin(graph: &Arc<Graph>, terminal: bool) -> Arc<Drain> {
    let (drain, fibers, bindings, effects) = {
        let mut table = graph.table.lock().unwrap();
        table.terminal |= terminal;
        if let Some(drain) = &table.drain {
            return drain.clone();
        }
        table.closing = true;
        let drain = Arc::new(Drain::default());
        table.drain = Some(drain.clone());
        let fibers: Vec<_> = table.fibers.values().cloned().map(Fiber::from_inner).collect();
        let bindings: Vec<_> = table
            .bindings
            .values()
            .filter(|binding| binding.definition.owner.is_none())
            .cloned()
            .map(|binding| BindingLease { binding, graph: graph.clone() })
            .collect();
        (drain, fibers, bindings, graph.effects.lock().unwrap().clone())
    };
    for fiber in &fibers {
        drop(fiber.begin_dispose());
    }
    let retiring: Vec<_> = bindings.iter().map(BindingLease::withdraw).collect();
    let worker = graph.clone();
    let completed = drain.clone();
    tokio::spawn(async move {
        let (_, mut errors, _) = tokio::join!(
            futures_util::future::join_all(retiring),
            effects.close(),
            futures_util::future::join_all(fibers.iter().map(Fiber::dispose))
        );
        errors.append(&mut *worker.diagnostics.lock().unwrap());
        {
            let mut table = worker.table.lock().unwrap();
            table.aliases.clear();
            if !table.terminal {
                *worker.effects.lock().unwrap() = EffectScope::new();
                table.closing = false;
                table.drain = None;
            }
        }
        completed.finish(ShutdownReport { closed: true, errors, retained: Vec::new() });
    });
    drain
}
