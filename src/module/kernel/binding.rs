use super::context::{Graph, Slot};
use super::fiber::Activation;
use super::{Context, Task};
use anyhow::{Result, ensure};
use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};
use tokio::sync::Notify;

pub(crate) struct Binding {
    pub id: u64,
    pub slot: Slot,
    pub value: RwLock<Option<Arc<dyn Any + Send + Sync>>>,
    pub available: AtomicBool,
    pub retiring: AtomicBool,
    pub retired: AtomicBool,
    pub completed: Notify,
    pub definition: Context,
    pub dependents: Mutex<Vec<Weak<Activation>>>,
}

impl Binding {
    pub(crate) async fn drain_dependents(&self) {
        let dependents: Vec<_> = self.dependents.lock().unwrap().iter().filter_map(Weak::upgrade).collect();
        futures_util::future::join_all(dependents.iter().map(|activation| activation.finished())).await;
    }

    pub fn ready(&self) -> bool {
        self.available.load(Ordering::Acquire)
            && self
                .definition
                .owner
                .as_ref()
                .is_none_or(|owner| owner.upgrade().is_some_and(|owner| owner.ready.load(Ordering::Acquire)))
    }
}

#[derive(Clone)]
pub struct BindingLease {
    pub(crate) binding: Arc<Binding>,
    pub(crate) graph: Arc<Graph>,
}

impl BindingLease {
    pub fn set<T: Any + Send + Sync>(&self, value: T) -> Result<()> {
        ensure!(self.binding.available.load(Ordering::Acquire), "binding is retired");
        let mut current = self.binding.value.write().unwrap();
        ensure!(self.binding.available.load(Ordering::Acquire), "binding is retired");
        ensure!(
            current.as_ref().is_some_and(|value| value.as_ref().type_id() == std::any::TypeId::of::<T>()),
            "service type mismatch"
        );
        let previous = current.replace(Arc::new(value));
        drop(current);
        drop(previous);
        Ok(())
    }

    pub fn withdraw(&self) -> Task<Result<()>> {
        self.withdraw_inner(false)
    }

    pub(crate) fn withdraw_owned(&self) -> Task<Result<()>> {
        self.withdraw_inner(true)
    }

    fn withdraw_inner(&self, retain_owner: bool) -> Task<Result<()>> {
        let start = {
            let mut table = self.graph.table.lock().unwrap();
            self.binding.available.store(false, Ordering::Release);
            if table.bindings.get(&self.binding.slot).is_some_and(|binding| binding.id == self.binding.id) {
                table.bindings.remove(&self.binding.slot);
            }
            !self.binding.retiring.swap(true, Ordering::AcqRel)
        };
        self.graph.changed();
        if start {
            let binding = self.binding.clone();
            tokio::spawn(async move {
                binding.drain_dependents().await;
                if let Some(owner) = binding.definition.owner.as_ref().and_then(Weak::upgrade).filter(|_| retain_owner)
                {
                    owner.finished().await;
                }
                let value = binding.value.write().unwrap().take();
                drop(value);
                binding.retired.store(true, Ordering::Release);
                binding.completed.notify_waiters();
            });
        }
        let binding = self.binding.clone();
        Box::pin(async move {
            loop {
                let completed = binding.completed.notified();
                tokio::pin!(completed);
                completed.as_mut().enable();
                if binding.retired.load(Ordering::Acquire) {
                    return Ok(());
                }
                completed.await;
            }
        })
    }
}
