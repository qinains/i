use super::Task;
use anyhow::{Result, anyhow, ensure};
use futures_util::FutureExt;
use futures_util::future::join_all;
use std::future::Future;
use std::sync::{Arc, Mutex, Weak};
use tokio::sync::Notify;

type Inverse = Box<dyn FnOnce() -> Task<Result<()>> + Send>;
enum EntryState {
    Setup,
    Ready(Option<Inverse>),
    Closing,
    Done(Result<(), String>),
}
struct Entry {
    owner: Weak<Scope>,
    state: Mutex<EntryState>,
    changed: Notify,
}

struct SetupGuard(Arc<Entry>);
impl Drop for SetupGuard {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap();
        let cancelled = matches!(*state, EntryState::Setup);
        if cancelled {
            *state = EntryState::Done(Ok(()));
        }
        drop(state);
        if cancelled {
            unlink(&self.0);
        }
        self.0.changed.notify_waiters();
    }
}

#[derive(Clone)]
pub struct EffectLease(Arc<Entry>);

impl EffectLease {
    pub async fn dispose(&self) -> Result<()> {
        loop {
            let changed = self.0.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let inverse = {
                let mut state = self.0.state.lock().unwrap();
                match &mut *state {
                    EntryState::Done(result) => return result.clone().map_err(|error| anyhow!(error)),
                    EntryState::Ready(inverse) => {
                        let inverse = inverse.take();
                        *state = EntryState::Closing;
                        inverse
                    }
                    _ => None,
                }
            };
            if let Some(inverse) = inverse {
                let mut task: Task<Result<()>> = Box::pin(async move {
                    std::panic::AssertUnwindSafe(async move { inverse().await })
                        .catch_unwind()
                        .await
                        .map_err(|_| anyhow!("effect disposer panicked"))?
                });
                // Preserve reverse start order; only the suspended continuation moves to a task.
                let first = std::future::poll_fn(|cx| std::task::Poll::Ready(task.as_mut().poll(cx))).await;
                let entry = self.0.clone();
                match first {
                    std::task::Poll::Ready(result) => finish(&entry, result),
                    std::task::Poll::Pending => {
                        tokio::spawn(async move {
                            finish(&entry, task.await);
                        });
                    }
                }
            }
            changed.await;
        }
    }
}

fn finish(entry: &Entry, result: Result<()>) {
    *entry.state.lock().unwrap() = EntryState::Done(result.map_err(|error| error.to_string()));
    unlink(entry);
    entry.changed.notify_waiters();
}

fn unlink(entry: &Entry) {
    if let Some(owner) = entry.owner.upgrade() {
        owner.state.lock().unwrap().entries.retain(|lease| !std::ptr::eq(lease.0.as_ref(), entry));
    }
}

#[derive(Default)]
struct ScopeState {
    entries: Vec<EffectLease>,
    closing: bool,
    result: Option<Vec<String>>,
}
struct Scope {
    state: Mutex<ScopeState>,
    changed: Notify,
    serial: bool,
}

#[derive(Clone)]
pub struct EffectScope(Arc<Scope>);

impl Default for EffectScope {
    fn default() -> Self {
        Self::new()
    }
}
impl EffectScope {
    pub fn new() -> Self {
        Self::with_order(false)
    }
    fn with_order(serial: bool) -> Self {
        Self(Arc::new(Scope { state: Mutex::default(), changed: Notify::new(), serial }))
    }
    pub fn is_closing(&self) -> bool {
        self.0.state.lock().unwrap().closing
    }
    pub(crate) fn closed() -> Self {
        Self(Arc::new(Scope {
            state: Mutex::new(ScopeState { closing: true, result: Some(Vec::new()), ..ScopeState::default() }),
            changed: Notify::new(),
            serial: false,
        }))
    }
    pub(crate) fn own<D, I>(&self, dispose: D) -> Result<EffectLease>
    where
        D: FnOnce() -> I + Send + 'static,
        I: Future<Output = Result<()>> + Send + 'static,
    {
        let lease = EffectLease(Arc::new(Entry {
            owner: Arc::downgrade(&self.0),
            state: Mutex::new(EntryState::Ready(Some(Box::new(move || Box::pin(dispose()))))),
            changed: Notify::new(),
        }));
        let mut state = self.0.state.lock().unwrap();
        ensure!(!state.closing, "effect scope is closing");
        state.entries.push(lease.clone());
        Ok(lease)
    }

    pub async fn acquire<S, F, D, I>(&self, setup: S) -> Result<EffectLease>
    where
        S: FnOnce() -> F,
        F: Future<Output = Result<D>> + Send,
        D: FnOnce() -> I + Send + 'static,
        I: Future<Output = Result<()>> + Send + 'static,
    {
        let lease = EffectLease(Arc::new(Entry {
            owner: Arc::downgrade(&self.0),
            state: Mutex::new(EntryState::Setup),
            changed: Notify::new(),
        }));
        {
            let mut scope = self.0.state.lock().unwrap();
            ensure!(!scope.closing, "effect scope is closing");
            scope.entries.push(lease.clone());
        }
        let _setup = SetupGuard(lease.0.clone());
        match setup().await {
            Ok(dispose) => {
                *lease.0.state.lock().unwrap() = EntryState::Ready(Some(Box::new(move || Box::pin(dispose()))));
                lease.0.changed.notify_waiters();
                if self.is_closing() {
                    lease.dispose().await?;
                }
                Ok(lease)
            }
            Err(error) => {
                finish(&lease.0, Ok(()));
                Err(error)
            }
        }
    }

    pub async fn group(&self) -> Result<Self> {
        let group = Self::with_order(true);
        let owned = group.clone();
        self.acquire(|| async move {
            Ok(move || async move {
                let errors = owned.close().await;
                ensure!(errors.is_empty(), "effect group cleanup: {}", errors.join("; "));
                Ok(())
            })
        })
        .await?;
        Ok(group)
    }

    pub fn close(&self) -> Task<Vec<String>> {
        let scope = self.clone();
        let entries = {
            let mut state = self.0.state.lock().unwrap();
            if state.closing {
                None
            } else {
                state.closing = true;
                Some(std::mem::take(&mut state.entries))
            }
        };
        // A caller dropping its waiter must not cancel cleanup for every other owner.
        if let Some(entries) = entries {
            let worker = self.clone();
            tokio::spawn(async move {
                let mut errors = Vec::new();
                if worker.0.serial {
                    for entry in entries.into_iter().rev() {
                        if let Err(error) = entry.dispose().await {
                            errors.push(error.to_string());
                        }
                    }
                } else {
                    for outcome in join_all(entries.iter().rev().map(EffectLease::dispose)).await {
                        if let Err(error) = outcome {
                            errors.push(error.to_string());
                        }
                    }
                }
                worker.0.state.lock().unwrap().result = Some(errors);
                worker.0.changed.notify_waiters();
            });
        }
        Box::pin(async move {
            loop {
                let changed = scope.0.changed.notified();
                tokio::pin!(changed);
                changed.as_mut().enable();
                if let Some(result) = scope.0.state.lock().unwrap().result.clone() {
                    return result;
                }
                changed.await;
            }
        })
    }
}
