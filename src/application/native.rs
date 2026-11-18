use crate::agent::{self, AgentHost};
use crate::module::kernel::{
    ActivationContext, BindingLease, Cancellation, Component, Context, Fiber, ServiceAccess, ServiceKey, ShutdownReport,
};
use anyhow::{Context as _, Result, ensure};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

pub struct Conversation {
    history: Vec<Value>,
    application: u64,
    connection: u64,
}
impl Conversation {
    pub fn history(&self) -> &[Value] {
        &self.history
    }
    pub fn into_history(self) -> Vec<Value> {
        self.history
    }
}

pub struct Services<P, W> {
    provider: ServiceAccess<P>,
    workspace: ServiceAccess<W>,
    activation: Cancellation,
    connection: Cancellation,
}
impl<P, W> Services<P, W> {
    pub fn provider(&self) -> &ServiceAccess<P> {
        &self.provider
    }
    pub fn workspace(&self) -> &ServiceAccess<W> {
        &self.workspace
    }
    pub fn is_cancelled(&self) -> bool {
        self.activation.is_cancelled() || self.connection.is_cancelled()
    }
    pub async fn cancelled(&self) {
        tokio::select! {_ = self.activation.cancelled() => {}, _ = self.connection.cancelled() => {}}
    }
}

pub trait ApplicationHost<P, W> {
    async fn before_turn(&mut self, _services: &Services<P, W>, _history: &mut Vec<Value>) -> Result<()> {
        Ok(())
    }
    async fn complete(&mut self, services: &Services<P, W>, history: &[Value]) -> Result<Value>;
    async fn before_tools(&mut self, _services: &Services<P, W>, _history: &[Value], _pending: usize) -> Result<usize> {
        Ok(usize::MAX)
    }
    async fn before_dispatch(&mut self, _services: &Services<P, W>, _history: &[Value], _call: &Value) -> Result<()> {
        Ok(())
    }
    async fn dispatch(&mut self, services: &Services<P, W>, call: &Value, slots: usize) -> Result<Value>;
    async fn after_dispatch(
        &mut self,
        _services: &Services<P, W>,
        _history: &[Value],
        _call: &Value,
        _result: &Value,
    ) -> Result<()> {
        Ok(())
    }
    async fn checkpoint(&mut self, _services: &Services<P, W>, _history: &mut Vec<Value>) -> Result<()> {
        Ok(())
    }
    async fn finish(&mut self, _services: &Services<P, W>, _history: &[Value]) -> Result<()> {
        Ok(())
    }
    fn emit(&mut self, _event: Value) -> Result<()> {
        Ok(())
    }
    fn cancelled(&self) -> bool {
        false
    }
}

struct Slot {
    context: oneshot::Sender<ActivationContext>,
    finished: oneshot::Receiver<()>,
}
type Slots = Arc<Mutex<HashMap<u64, Slot>>>;
struct Configuration {
    provider: BindingLease,
    generation: u64,
    cancellation: Cancellation,
}

pub struct AgentApplication<P, W> {
    id: u64,
    context: Context,
    configuration: Mutex<Configuration>,
    run_component: Arc<Component>,
    slots: Slots,
    sequence: AtomicU64,
    workspace: std::marker::PhantomData<fn() -> W>,
    provider: std::marker::PhantomData<fn() -> P>,
}

impl<P: Send + Sync + 'static, W: Send + Sync + 'static> AgentApplication<P, W> {
    pub fn new(provider: Arc<P>, workspace: Arc<W>) -> Result<Self> {
        let context = Context::new();
        let binding = context.provide_arc(ServiceKey::<P>::new("provider"), provider)?;
        context.provide_arc(ServiceKey::<W>::new("workspace"), workspace)?;
        let slots: Slots = Arc::default();
        let pending = slots.clone();
        let run_component = Arc::new(
            Component::new(move |ctx, config| {
                let pending = pending.clone();
                async move {
                    let id = config["run"].as_u64().context("missing run identity")?;
                    let slot = pending.lock().unwrap().remove(&id).context("run entry has expired")?;
                    slot.context.send(ctx).map_err(|_| anyhow::anyhow!("run caller was dropped"))?;
                    let _ = slot.finished.await;
                    Ok(())
                }
            })
            .requires(ServiceKey::<P>::new("provider"))
            .requires(ServiceKey::<W>::new("workspace")),
        );
        Ok(Self {
            id: crate::module::kernel::next_id(),
            context,
            configuration: Mutex::new(Configuration {
                provider: binding,
                generation: 0,
                cancellation: Cancellation::default(),
            }),
            run_component,
            slots,
            sequence: AtomicU64::new(1),
            workspace: std::marker::PhantomData,
            provider: std::marker::PhantomData,
        })
    }
    pub fn context(&self) -> &Context {
        &self.context
    }
    pub fn attach_conversation(&self, history: Vec<Value>) -> Result<Conversation> {
        let state = self.configuration.lock().unwrap();
        ensure!(!self.context.is_closing(), "application is closed");
        Ok(Conversation {
            history: agent::history::checked_history(json!(history))?,
            application: self.id,
            connection: state.generation,
        })
    }
    pub fn reattach_conversation(&self, conversation: &mut Conversation) -> Result<()> {
        ensure!(!self.context.is_closing(), "application is closed");
        conversation.application = self.id;
        conversation.connection = self.configuration.lock().unwrap().generation;
        Ok(())
    }
    pub async fn replace_provider(&self, provider: Arc<P>) -> Result<()> {
        let retiring = {
            let mut state = self.configuration.lock().unwrap();
            ensure!(!self.context.is_closing(), "application is closed");
            let retiring = state.provider.withdraw();
            state.cancellation.cancel();
            state.generation += 1;
            state.cancellation = Cancellation::default();
            state.provider = self.context.provide_arc(ServiceKey::<P>::new("provider"), provider)?;
            retiring
        };
        retiring.await
    }
    pub async fn run<H: ApplicationHost<P, W>>(
        &self,
        host: &mut H,
        conversation: &mut Conversation,
        max_steps: usize,
    ) -> Result<()> {
        agent::history::checked_history(json!(conversation.history))?;
        let id = self.sequence.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = oneshot::channel();
        let (finished, completion) = oneshot::channel();
        let (fiber, connection) = {
            let state = self.configuration.lock().unwrap();
            ensure!(!self.context.is_closing(), "application is closed");
            ensure!(
                conversation.application == self.id && conversation.connection == state.generation,
                "conversation belongs to another provider; explicitly attach it before running"
            );
            self.slots.lock().unwrap().insert(id, Slot { context: sender, finished: completion });
            let fiber = match self.context.mount(self.run_component.clone(), json!({"run": id})) {
                Ok(fiber) => fiber,
                Err(error) => {
                    self.slots.lock().unwrap().remove(&id);
                    return Err(error);
                }
            };
            (fiber, state.cancellation.clone())
        };
        let guard = RunGuard { fiber, finished: Some(finished), slots: self.slots.clone(), id };
        let ctx = tokio::select! {biased; _ = connection.cancelled() => anyhow::bail!("provider changed before run activation"), _ = guard.fiber.disposed() => anyhow::bail!("run activation was disposed"), ctx = receiver => ctx.context("run activation was cancelled")?};
        let services = Services {
            provider: ctx.service(ServiceKey::<P>::new("provider"))?,
            workspace: ctx.service(ServiceKey::<W>::new("workspace"))?,
            activation: ctx.cancellation(),
            connection,
        };
        let mut scoped = ScopedHost { host, services: &services };
        let result = agent::run(&mut scoped, &mut conversation.history, max_steps).await;
        let fiber = guard.fiber.clone();
        drop(guard);
        fiber.dispose().await?;
        result
    }
    pub fn shutdown(&self) -> impl std::future::Future<Output = ShutdownReport> + Send + 'static {
        self.configuration.lock().unwrap().cancellation.cancel();
        self.context.shutdown()
    }
}

impl<P, W> Drop for AgentApplication<P, W> {
    fn drop(&mut self) {
        self.configuration.lock().unwrap().cancellation.cancel();
        if tokio::runtime::Handle::try_current().is_ok() {
            self.context.request_shutdown();
        }
    }
}

struct RunGuard {
    fiber: Fiber,
    finished: Option<oneshot::Sender<()>>,
    slots: Slots,
    id: u64,
}
impl Drop for RunGuard {
    fn drop(&mut self) {
        drop(self.fiber.begin_dispose());
        self.slots.lock().unwrap().remove(&self.id);
        if let Some(finished) = self.finished.take() {
            let _ = finished.send(());
        }
    }
}

struct ScopedHost<'a, H, P, W> {
    host: &'a mut H,
    services: &'a Services<P, W>,
}
impl<H: ApplicationHost<P, W>, P, W> AgentHost for ScopedHost<'_, H, P, W> {
    async fn before_turn(&mut self, history: &mut Vec<Value>) -> Result<()> {
        self.host.before_turn(self.services, history).await
    }
    async fn complete(&mut self, history: &[Value]) -> Result<Value> {
        tokio::select! {biased; _ = self.services.cancelled() => anyhow::bail!("cancelled"), result = self.host.complete(self.services, history) => result}
    }
    async fn before_tools(&mut self, history: &[Value], pending: usize) -> Result<usize> {
        self.host.before_tools(self.services, history, pending).await
    }
    async fn before_dispatch(&mut self, history: &[Value], call: &Value) -> Result<()> {
        self.host.before_dispatch(self.services, history, call).await
    }
    async fn dispatch(&mut self, call: &Value, slots: usize) -> Result<Value> {
        ensure!(!self.cancelled(), "cancelled");
        self.host.dispatch(self.services, call, slots).await
    }
    async fn after_dispatch(&mut self, history: &[Value], call: &Value, result: &Value) -> Result<()> {
        self.host.after_dispatch(self.services, history, call, result).await
    }
    async fn checkpoint(&mut self, history: &mut Vec<Value>) -> Result<()> {
        self.host.checkpoint(self.services, history).await
    }
    async fn finish(&mut self, history: &[Value]) -> Result<()> {
        self.host.finish(self.services, history).await
    }
    fn emit(&mut self, event: Value) -> Result<()> {
        self.host.emit(event)
    }
    fn cancelled(&self) -> bool {
        self.services.is_cancelled() || self.host.cancelled()
    }
}
