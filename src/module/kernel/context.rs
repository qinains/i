use super::binding::{Binding, BindingLease};
use super::fiber::{Activation, Fiber, FiberId, FiberInner};
use super::service::{Intercept, ServiceAccess, ServiceKey};
use super::{Cancellation, Component, ComponentId, EffectScope, next_id};
use anyhow::{Result, anyhow, ensure};
use serde_json::Value;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct RealmId(u64);
impl RealmId {
    pub fn new() -> Self {
        Self(next_id())
    }
}

#[derive(Clone, Eq, Hash, PartialEq)]
pub(crate) struct Slot {
    pub name: Arc<str>,
    pub realm: RealmId,
}

#[derive(Default)]
pub(crate) struct Table {
    pub closing: bool,
    pub terminal: bool,
    pub drain: Option<Arc<super::shutdown::Drain>>,
    pub bindings: HashMap<Slot, Arc<Binding>>,
    pub fibers: HashMap<FiberId, Arc<FiberInner>>,
    pub aliases: HashMap<ComponentId, Arc<Component>>,
}

#[derive(Default)]
pub(crate) struct Graph {
    pub table: Mutex<Table>,
    pub effects: Mutex<EffectScope>,
    pub diagnostics: Mutex<Vec<String>>,
}

impl Graph {
    pub fn changed(&self) {
        let fibers: Vec<_> = self.table.lock().unwrap().fibers.values().cloned().collect();
        for fiber in fibers {
            fiber.request();
        }
    }
}

#[derive(Clone)]
pub struct Context {
    pub(crate) graph: Arc<Graph>,
    pub(crate) owner: Option<Weak<Activation>>,
    pub(crate) realms: Arc<HashMap<Arc<str>, RealmId>>,
    pub(crate) intercepts: Arc<Vec<Intercept>>,
}

#[derive(Clone)]
pub struct ActivationContext(pub(crate) Context);

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}
impl Context {
    pub fn instances(&self, component: ComponentId) -> Vec<FiberId> {
        self.graph
            .table
            .lock()
            .unwrap()
            .fibers
            .values()
            .filter_map(|fiber| fiber.registered_as(component).then_some(fiber.id()))
            .collect()
    }
    pub fn is_closing(&self) -> bool {
        self.graph.table.lock().unwrap().closing
    }
    pub fn new() -> Self {
        Self { graph: Arc::new(Graph::default()), owner: None, realms: Arc::default(), intercepts: Arc::default() }
    }
    pub fn extend(&self) -> Self {
        self.clone()
    }
    /// Derive a metadata view, not a new lifetime or service realm.
    pub fn intercept<T, M: Send + Sync + 'static>(&self, key: ServiceKey<T>, metadata: M) -> Self {
        let mut intercepts = (*self.intercepts).clone();
        intercepts.push(Intercept { service: key.name, value: Arc::new(metadata) });
        Self { intercepts: Arc::new(intercepts), ..self.clone() }
    }
    pub fn effects(&self) -> EffectScope {
        match &self.owner {
            None => {
                if self.graph.table.lock().unwrap().closing {
                    EffectScope::closed()
                } else {
                    self.graph.effects.lock().unwrap().clone()
                }
            }
            Some(owner) => owner.upgrade().map_or_else(EffectScope::closed, |owner| owner.effects.clone()),
        }
    }
    pub fn owner_id(&self) -> Option<FiberId> {
        self.owner.as_ref()?.upgrade().map(|activation| activation.fiber)
    }
    pub fn isolate<T>(&self, key: ServiceKey<T>, realm: RealmId) -> Self {
        let mut realms = (*self.realms).clone();
        realms.insert(key.name, realm);
        Self { realms: Arc::new(realms), ..self.clone() }
    }
    pub(crate) fn slot(&self, name: Arc<str>) -> Slot {
        Slot { realm: self.realms.get(&name).copied().unwrap_or_default(), name }
    }
    pub fn mount(&self, component: Arc<Component>, config: Value) -> Result<Fiber> {
        let owner = self
            .owner
            .as_ref()
            .map(|owner| owner.upgrade().ok_or_else(|| anyhow!("activation is closed")))
            .transpose()?;
        let mut children = owner.as_ref().map(|owner| owner.children.lock().unwrap());
        ensure!(owner.as_ref().is_none_or(|owner| !owner.closed.load(Ordering::Acquire)), "activation is closed");
        let mut component = component;
        {
            let table = self.graph.table.lock().unwrap();
            ensure!(!table.closing, "application is closing");
            while let Some(next) = table.aliases.get(&component.id()) {
                component = next.clone();
            }
        }
        let fiber = Fiber::mount(self.clone(), component, config)?;
        let owned = fiber.clone();
        match self.effects().own(move || async move { owned.dispose().await }) {
            Ok(lease) => fiber.adopt(lease),
            Err(error) => {
                drop(fiber.begin_dispose());
                return Err(error);
            }
        }
        if let Some(children) = &mut children {
            children.push(fiber.clone());
        }
        Ok(fiber)
    }
    pub fn replace_component(&self, id: ComponentId, replacement: Arc<Component>) -> Result<()> {
        let (id, fibers) = {
            let mut table = self.graph.table.lock().unwrap();
            ensure!(!table.closing, "application is closing");
            let mut id = id;
            while let Some(next) = table.aliases.get(&id) {
                id = next.id();
            }
            table.aliases.remove(&replacement.id());
            if id != replacement.id() {
                table.aliases.insert(id, replacement.clone());
            }
            (id, table.fibers.values().cloned().collect::<Vec<_>>())
        };
        for fiber in fibers {
            fiber.replace(id, replacement.clone());
        }
        Ok(())
    }
    pub fn provide<T: Send + Sync + 'static>(&self, key: ServiceKey<T>, value: T) -> Result<BindingLease> {
        self.provide_arc(key, Arc::new(value))
    }
    pub(crate) fn provide_arc<T: Send + Sync + 'static>(
        &self,
        key: ServiceKey<T>,
        value: Arc<T>,
    ) -> Result<BindingLease> {
        let owner = self
            .owner
            .as_ref()
            .map(|owner| owner.upgrade().ok_or_else(|| anyhow!("activation is closed")))
            .transpose()?;
        let mut provisions = owner.as_ref().map(|owner| owner.provisions.lock().unwrap());
        ensure!(owner.as_ref().is_none_or(|owner| !owner.closed.load(Ordering::Acquire)), "activation is closed");
        let slot = self.slot(key.name);
        let binding = Arc::new(Binding {
            id: next_id(),
            slot: slot.clone(),
            value: RwLock::new(Some(value)),
            available: AtomicBool::new(true),
            retiring: AtomicBool::new(false),
            retired: AtomicBool::new(false),
            completed: tokio::sync::Notify::new(),
            definition: self.clone(),
            dependents: Mutex::new(Vec::new()),
        });
        {
            let mut table = self.graph.table.lock().unwrap();
            ensure!(!table.closing, "application is closing");
            ensure!(!table.bindings.contains_key(&slot), "service is already provided");
            table.bindings.insert(slot, binding.clone());
        }
        if let Some(provisions) = &mut provisions {
            provisions.push(binding.clone());
        }
        drop(provisions);
        self.graph.changed();
        Ok(BindingLease { binding, graph: self.graph.clone() })
    }
    pub fn service<T: Send + Sync + 'static>(&self, key: ServiceKey<T>) -> Result<ServiceAccess<T>> {
        let slot = self.slot(key.name);
        let (binding, injected) = if let Some(owner) = self.owner.as_ref() {
            let mut owner = owner.upgrade().ok_or_else(|| anyhow!("activation is no longer alive"))?;
            loop {
                ensure!(!owner.done.load(Ordering::Acquire), "activation has finished");
                if let Some(binding) =
                    owner.provisions.lock().unwrap().iter().rev().find(|binding| binding.slot == slot).cloned()
                {
                    break (binding, None);
                }
                ensure!(owner.context.slot(slot.name.clone()) == slot, "service isolation blocks inherited injection");
                if let Some(binding) = owner.dependencies.get(&slot).cloned() {
                    break (binding, owner.injected.get(&slot).cloned());
                }
                let parent = owner
                    .parent
                    .owner
                    .as_ref()
                    .and_then(Weak::upgrade)
                    .ok_or_else(|| anyhow!("service not admitted in committed activation"))?;
                owner = parent;
            }
        } else {
            let binding = self
                .graph
                .table
                .lock()
                .unwrap()
                .bindings
                .get(&slot)
                .cloned()
                .ok_or_else(|| anyhow!("service is unavailable"))?;
            (binding, None)
        };
        Ok(ServiceAccess { binding, caller: ActivationContext(self.clone()), injected, marker: PhantomData })
    }
}

impl ActivationContext {
    pub fn cancellation(&self) -> Cancellation {
        self.0.owner.as_ref().and_then(Weak::upgrade).map_or_else(
            || {
                let token = Cancellation::default();
                token.cancel();
                token
            },
            |owner| owner.cancellation.clone(),
        )
    }
    pub fn effects(&self) -> EffectScope {
        self.0.effects()
    }
    pub fn context(&self) -> Context {
        self.0.clone()
    }
    pub fn service<T: Send + Sync + 'static>(&self, key: ServiceKey<T>) -> Result<ServiceAccess<T>> {
        self.0.service(key)
    }
    pub fn provide<T: Send + Sync + 'static>(&self, key: ServiceKey<T>, value: T) -> Result<BindingLease> {
        self.0.provide(key, value)
    }
}
