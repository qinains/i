use super::service::{Requirement, ServiceKey};
use super::{ActivationContext, Task, next_id};
use anyhow::Result;
use serde_json::Value;
use std::future::Future;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ComponentId(pub(crate) u64);

pub struct Component {
    pub(crate) id: ComponentId,
    pub(crate) inject: Vec<Requirement>,
    pub(crate) apply: Arc<dyn Fn(ActivationContext, Value) -> Task<Result<()>> + Send + Sync>,
}

impl Component {
    pub fn new<F, Fut>(apply: F) -> Self
    where
        F: Fn(ActivationContext, Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        Self {
            id: ComponentId(next_id()),
            inject: Vec::new(),
            apply: Arc::new(move |ctx, config| Box::pin(apply(ctx, config))),
        }
    }

    pub fn id(&self) -> ComponentId {
        self.id
    }

    pub fn requires<T: Send + Sync + 'static>(mut self, key: ServiceKey<T>) -> Self {
        self.inject.push(key.requirement());
        self
    }

    pub fn requires_with<T: Send + Sync + 'static, M: Send + Sync + 'static>(
        mut self,
        key: ServiceKey<T>,
        metadata: M,
    ) -> Self {
        let mut requirement = key.requirement();
        requirement.metadata = Some(Arc::new(metadata));
        self.inject.push(requirement);
        self
    }
}
