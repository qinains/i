use super::binding::Binding;
use super::{ActivationContext, Context};
use anyhow::{Result, anyhow};
use std::any::{Any, TypeId};
use std::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;

pub(crate) type Metadata = Arc<dyn Any + Send + Sync>;

#[derive(Clone)]
pub(crate) struct Intercept {
    pub service: Arc<str>,
    pub value: Metadata,
}

pub struct ServiceKey<T> {
    pub(crate) name: Arc<str>,
    marker: PhantomData<fn() -> T>,
}
impl<T> Clone for ServiceKey<T> {
    fn clone(&self) -> Self {
        Self { name: self.name.clone(), marker: PhantomData }
    }
}

#[derive(Clone)]
pub(crate) struct Requirement {
    pub name: Arc<str>,
    pub kind: TypeId,
    pub metadata: Option<Metadata>,
}

impl<T: Send + Sync + 'static> ServiceKey<T> {
    pub fn new(name: impl Into<Arc<str>>) -> Self {
        Self { name: name.into(), marker: PhantomData }
    }
    pub(crate) fn requirement(&self) -> Requirement {
        Requirement { name: self.name.clone(), kind: TypeId::of::<T>(), metadata: None }
    }
}

pub struct ServiceAccess<T> {
    pub(crate) binding: Arc<Binding>,
    pub(crate) caller: ActivationContext,
    pub(crate) injected: Option<Metadata>,
    pub(crate) marker: PhantomData<fn() -> T>,
}

pub struct ServiceCall {
    pub definition: Context,
    pub caller: ActivationContext,
    service: Arc<str>,
    injected: Option<Metadata>,
}

impl ServiceCall {
    /// Merge declaration metadata followed by caller intercepts, from outer to inner.
    pub fn metadata<M: Send + Sync + 'static, R>(&self, merge: impl FnOnce(&[&M]) -> R) -> Result<R> {
        let values = self.injected.iter().chain(
            self.caller.0.intercepts.iter().filter(|entry| entry.service == self.service).map(|entry| &entry.value),
        );
        let layers = values
            .map(|value| value.downcast_ref::<M>().ok_or_else(|| anyhow!("interception metadata type mismatch")))
            .collect::<Result<Vec<_>>>()?;
        Ok(merge(&layers))
    }

    /// Resolve at the definition site without losing the original caller's effects/intercepts.
    pub fn service<T: Send + Sync + 'static>(&self, key: ServiceKey<T>) -> Result<ServiceAccess<T>> {
        let mut access = self.definition.service(key)?;
        access.caller = self.caller.clone();
        Ok(access)
    }
}

pub struct ServiceBorrow<'scope, T> {
    value: Arc<T>,
    scope: PhantomData<&'scope ()>,
}
impl<T> std::ops::Deref for ServiceBorrow<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T: Send + Sync + 'static> ServiceAccess<T> {
    fn scope(&self) -> ServiceCall {
        ServiceCall {
            definition: self.binding.definition.clone(),
            caller: self.caller.clone(),
            service: self.binding.slot.name.clone(),
            injected: self.injected.clone(),
        }
    }
    fn value(&self) -> Result<Arc<T>> {
        if let Some(owner) = &self.caller.0.owner {
            anyhow::ensure!(
                owner.upgrade().is_some_and(|owner| !owner.done.load(std::sync::atomic::Ordering::Acquire)),
                "caller activation has finished"
            );
        } else {
            anyhow::ensure!(!self.caller.0.is_closing() && self.binding.ready(), "service is no longer available");
        }
        let value = self
            .binding
            .value
            .read()
            .unwrap()
            .clone()
            .ok_or_else(|| anyhow!("binding resources have been released"))?;
        value.downcast::<T>().map_err(|_| anyhow!("service type mismatch"))
    }
    pub fn call<R>(&self, callback: impl FnOnce(&T, ServiceCall) -> R) -> Result<R> {
        let service = self.value()?;
        Ok(callback(&service, self.scope()))
    }
    pub async fn call_async<'scope, R, F, Fut>(&'scope self, callback: F) -> Result<R>
    where
        F: FnOnce(ServiceBorrow<'scope, T>, ServiceCall) -> Fut,
        Fut: Future<Output = Result<R>> + 'scope,
    {
        let service = ServiceBorrow { value: self.value()?, scope: PhantomData };
        callback(service, self.scope()).await
    }
}
