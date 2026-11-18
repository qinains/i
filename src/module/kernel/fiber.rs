use super::binding::{Binding, BindingLease};
use super::context::{Context, Slot};
use super::{ActivationContext, Cancellation, Component, ComponentId, EffectLease, EffectScope, Task, next_id};
use anyhow::{Result, anyhow};
use futures_util::FutureExt;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FiberId(pub(crate) u64);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FiberState {
    Pending,
    Loading,
    Active,
    Failed,
    Unloading,
    Disposed,
}

pub(crate) struct Activation {
    pub cancellation: Cancellation,
    revision: u64,
    component: ComponentId,
    pub fiber: FiberId,
    pub context: Context,
    pub parent: Context,
    pub dependencies: HashMap<Slot, Arc<Binding>>,
    pub injected: HashMap<Slot, super::service::Metadata>,
    pub provisions: Mutex<Vec<Arc<Binding>>>,
    pub closed: AtomicBool,
    pub effects: EffectScope,
    pub ready: AtomicBool,
    pub done: AtomicBool,
    pub completed: Notify,
    pub children: Mutex<Vec<Fiber>>,
}

impl Activation {
    pub async fn finished(&self) {
        loop {
            let completed = self.completed.notified();
            tokio::pin!(completed);
            completed.as_mut().enable();
            if self.done.load(Ordering::Acquire) {
                return;
            }
            completed.await;
        }
    }
}

struct Data {
    component: Arc<Component>,
    config: Value,
    revision: u64,
    applied: u64,
    epoch: u64,
    running: bool,
    disposed: bool,
    state: FiberState,
    activation: Option<Arc<Activation>>,
    loading: Option<std::sync::Weak<Activation>>,
    error: Option<String>,
}

pub(crate) struct FiberInner {
    owner: Mutex<Option<EffectLease>>,
    id: FiberId,
    parent: Context,
    data: Mutex<Data>,
    changed: Notify,
}

#[derive(Clone)]
pub struct Fiber(Arc<FiberInner>);

impl Fiber {
    pub(crate) fn adopt(&self, lease: EffectLease) {
        let mut owner = self.0.owner.lock().unwrap();
        if self.state() == FiberState::Disposed {
            tokio::spawn(async move {
                let _ = lease.dispose().await;
            });
        } else {
            *owner = Some(lease);
        }
    }
    pub(crate) fn from_inner(inner: Arc<FiberInner>) -> Self {
        Self(inner)
    }
    pub(crate) fn mount(parent: Context, component: Arc<Component>, config: Value) -> Result<Self> {
        let inner = Arc::new(FiberInner {
            owner: Mutex::new(None),
            id: FiberId(next_id()),
            parent: parent.clone(),
            changed: Notify::new(),
            data: Mutex::new(Data {
                component,
                config,
                revision: 0,
                applied: 0,
                epoch: 0,
                running: false,
                disposed: false,
                state: FiberState::Pending,
                activation: None,
                loading: None,
                error: None,
            }),
        });
        {
            let mut table = parent.graph.table.lock().unwrap();
            anyhow::ensure!(!table.closing, "application is closing");
            table.fibers.insert(inner.id, inner.clone());
        }
        inner.request();
        Ok(Self(inner))
    }
    pub fn id(&self) -> FiberId {
        self.0.id
    }
    pub fn state(&self) -> FiberState {
        self.0.data.lock().unwrap().state
    }
    pub async fn disposed(&self) {
        loop {
            let changed = self.0.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.state() == FiberState::Disposed {
                return;
            }
            changed.await;
        }
    }
    pub fn update(&self, config: Value) -> Result<()> {
        {
            let mut data = self.0.data.lock().unwrap();
            anyhow::ensure!(!data.disposed, "fiber is disposed");
            data.config = config;
            data.revision += 1;
            data.error = None;
        }
        self.0.request();
        Ok(())
    }
    pub async fn settled(&self) -> Result<FiberState> {
        loop {
            let changed = self.0.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            {
                let data = self.0.data.lock().unwrap();
                if !data.running {
                    return data.error.as_ref().map_or(Ok(data.state), |error| Err(anyhow!(error.clone())));
                }
            }
            changed.await;
        }
    }
    pub async fn dispose(&self) -> Result<()> {
        self.begin_dispose().await
    }
    pub(crate) fn begin_dispose(&self) -> Task<Result<()>> {
        {
            let mut data = self.0.data.lock().unwrap();
            if data.state == FiberState::Disposed && !data.running {
                return Box::pin(async { Ok(()) });
            }
            data.disposed = true;
        }
        self.0.request();
        let fiber = self.clone();
        Box::pin(async move { fiber.settled().await.map(|_| ()) })
    }
}

impl FiberInner {
    pub(crate) fn id(&self) -> FiberId {
        self.id
    }
    pub(crate) fn registered_as(&self, component: ComponentId) -> bool {
        let data = self.data.lock().unwrap();
        !data.disposed && data.component.id() == component
    }
    fn current(
        &self,
        table: &super::context::Table,
        data: &Data,
        activation: &Activation,
        revision: u64,
        component: ComponentId,
    ) -> bool {
        !table.closing
            && !data.disposed
            && data.revision == revision
            && data.component.id() == component
            && activation.dependencies.iter().all(|(slot, binding)| {
                binding.ready() && table.bindings.get(slot).is_some_and(|current| current.id == binding.id)
            })
    }
    pub fn replace(self: &Arc<Self>, id: ComponentId, replacement: Arc<Component>) {
        let replaced = {
            let mut data = self.data.lock().unwrap();
            if data.component.id() == id && !data.disposed {
                data.component = replacement;
                data.revision += 1;
                data.error = None;
                true
            } else {
                false
            }
        };
        if replaced {
            self.request();
        }
    }
    pub fn request(self: &Arc<Self>) {
        let (start, cancel_children) = {
            let table = self.parent.graph.table.lock().unwrap();
            let mut data = self.data.lock().unwrap();
            let mut cancelled = None;
            let activation =
                data.activation.clone().or_else(|| data.loading.as_ref().and_then(std::sync::Weak::upgrade));
            if let Some(activation) = activation {
                let inherited_cancel = self
                    .parent
                    .owner
                    .as_ref()
                    .and_then(std::sync::Weak::upgrade)
                    .is_some_and(|parent| parent.cancellation.is_cancelled());
                if (inherited_cancel
                    || !self.current(&table, &data, &activation, activation.revision, activation.component))
                    && activation.cancellation.cancel()
                {
                    cancelled = Some(activation);
                }
            }
            data.epoch += 1;
            let start = if data.running {
                false
            } else {
                data.running = true;
                true
            };
            (start, cancelled)
        };
        if let Some(activation) = cancel_children {
            let children = activation.children.lock().unwrap().clone();
            for child in children {
                child.0.request();
            }
        }
        if start {
            let this = self.clone();
            tokio::spawn(async move {
                this.reconcile().await;
            });
        }
    }

    fn resolve(&self, component: &Component) -> Option<HashMap<Slot, Arc<Binding>>> {
        let table = self.parent.graph.table.lock().unwrap();
        component
            .inject
            .iter()
            .map(|dependency| {
                let slot = self.parent.slot(dependency.name.clone());
                let binding = table.bindings.get(&slot)?;
                if !binding.ready()
                    || !binding
                        .value
                        .read()
                        .unwrap()
                        .as_ref()
                        .is_some_and(|value| value.as_ref().type_id() == dependency.kind)
                {
                    return None;
                }
                Some((slot, binding.clone()))
            })
            .collect()
    }

    async fn close(&self, activation: Arc<Activation>) {
        activation.cancellation.cancel();
        activation.ready.store(false, Ordering::Release);
        activation.closed.store(true, Ordering::Release);
        let children = std::mem::take(&mut *activation.children.lock().unwrap());
        for child in children {
            drop(child.begin_dispose());
        }
        let provisions = activation.provisions.lock().unwrap().clone();
        let draining: Vec<_> = provisions
            .iter()
            .map(|binding| {
                (BindingLease { binding: binding.clone(), graph: self.parent.graph.clone() }).withdraw_owned()
            })
            .collect();
        futures_util::future::join_all(provisions.iter().map(|binding| binding.drain_dependents())).await;
        for error in activation.effects.close().await {
            tracing::error!(%error, "component cleanup failed");
            self.parent.graph.diagnostics.lock().unwrap().push(error);
        }
        activation.done.store(true, Ordering::Release);
        activation.completed.notify_waiters();
        futures_util::future::join_all(draining).await;
        activation.provisions.lock().unwrap().clear();
    }

    async fn reconcile(self: Arc<Self>) {
        loop {
            let (epoch, disposed, component, config, revision, applied, activation, failed) = {
                let data = self.data.lock().unwrap();
                (
                    data.epoch,
                    data.disposed,
                    data.component.clone(),
                    data.config.clone(),
                    data.revision,
                    data.applied,
                    data.activation.clone(),
                    data.error.is_some(),
                )
            };
            let dependencies = if disposed || failed { None } else { self.resolve(&component) };
            let same = activation.as_ref().is_some_and(|active| {
                dependencies.as_ref().is_some_and(|next| {
                    next.len() == active.dependencies.len()
                        && next
                            .iter()
                            .all(|(key, binding)| active.dependencies.get(key).is_some_and(|old| old.id == binding.id))
                })
            });
            if let Some(active) = activation
                && (disposed || failed || !same || applied != revision)
            {
                self.data.lock().unwrap().state = FiberState::Unloading;
                self.close(active).await;
                self.data.lock().unwrap().activation = None;
                continue;
            }
            let needs_load = { self.data.lock().unwrap().activation.is_none() };
            if !disposed && !failed && needs_load {
                if let Some(dependencies) = dependencies {
                    let activation = Arc::new_cyclic(|owner| Activation {
                        cancellation: Cancellation::default(),
                        revision,
                        component: component.id(),
                        fiber: self.id,
                        context: Context { owner: Some(owner.clone()), ..self.parent.clone() },
                        parent: self.parent.clone(),
                        dependencies,
                        injected: component
                            .inject
                            .iter()
                            .filter_map(|requirement| {
                                requirement
                                    .metadata
                                    .clone()
                                    .map(|metadata| (self.parent.slot(requirement.name.clone()), metadata))
                            })
                            .collect(),
                        provisions: Mutex::new(Vec::new()),
                        closed: AtomicBool::new(false),
                        effects: EffectScope::new(),
                        ready: AtomicBool::new(false),
                        done: AtomicBool::new(false),
                        completed: Notify::new(),
                        children: Mutex::new(Vec::new()),
                    });
                    let committed = {
                        let table = self.parent.graph.table.lock().unwrap();
                        let mut data = self.data.lock().unwrap();
                        let valid = self.current(&table, &data, &activation, revision, component.id());
                        if valid {
                            data.loading = Some(Arc::downgrade(&activation));
                            for binding in activation.dependencies.values() {
                                binding.dependents.lock().unwrap().push(Arc::downgrade(&activation));
                            }
                        }
                        valid
                    };
                    if !committed {
                        continue;
                    }
                    self.data.lock().unwrap().state = FiberState::Loading;
                    let context = ActivationContext(activation.context.clone());
                    let implementation = component.apply.clone();
                    let result = std::panic::AssertUnwindSafe(async move { implementation(context, config).await })
                        .catch_unwind()
                        .await
                        .unwrap_or_else(|_| Err(anyhow!("component hook panicked")));
                    let published = {
                        let table = self.parent.graph.table.lock().unwrap();
                        let mut data = self.data.lock().unwrap();
                        let current = self.current(&table, &data, &activation, revision, component.id());
                        if current && result.is_ok() {
                            data.activation = Some(activation.clone());
                            data.applied = revision;
                            data.state = FiberState::Active;
                            activation.ready.store(true, Ordering::Release);
                            true
                        } else {
                            if current && let Err(error) = &result {
                                data.error = Some(error.to_string());
                            }
                            data.state = FiberState::Unloading;
                            false
                        }
                    };
                    if published {
                        self.parent.graph.changed();
                    } else {
                        self.close(activation).await;
                        let mut data = self.data.lock().unwrap();
                        data.state = if data.error.is_some() { FiberState::Failed } else { FiberState::Pending };
                    }
                    self.data.lock().unwrap().loading = None;
                } else {
                    self.data.lock().unwrap().state = FiberState::Pending;
                }
            }
            let finished = {
                let mut data = self.data.lock().unwrap();
                if data.epoch == epoch {
                    if disposed {
                        data.state = FiberState::Disposed;
                        data.error = None;
                    }
                    data.running = false;
                    true
                } else {
                    false
                }
            };
            if finished {
                if disposed {
                    self.parent.graph.table.lock().unwrap().fibers.remove(&self.id);
                    if let Some(owner) = self.owner.lock().unwrap().take() {
                        tokio::spawn(async move {
                            let _ = owner.dispose().await;
                        });
                    }
                }
                self.changed.notify_waiters();
                return;
            }
        }
    }
}
