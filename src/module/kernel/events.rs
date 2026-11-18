use super::{EffectLease, EffectScope, Task};
use anyhow::{Result, anyhow, ensure};
use futures_util::future::join_all;
use serde_json::Value;
use std::fmt;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

#[derive(Clone, Debug, PartialEq)]
pub enum EventValue {
    Undefined,
    Value(Value),
}
impl EventValue {
    fn bailed(&self) -> bool {
        !matches!(self, Self::Undefined | Self::Value(Value::Null | Value::Bool(false)))
    }
}

#[derive(Clone, Copy, Default)]
pub struct ListenOptions {
    pub once: bool,
    pub prepend: bool,
}

#[derive(Debug)]
pub struct ParallelErrors {
    pub errors: Vec<String>,
}
impl fmt::Display for ParallelErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "one or more event listeners failed: {}", self.errors.join("; "))
    }
}
impl std::error::Error for ParallelErrors {}

type Async<P> = Arc<dyn Fn(P) -> Task<Result<EventValue>> + Send + Sync>;
#[derive(Clone)]
enum Callback<P> {
    Sync(Arc<dyn Fn(P) -> Result<EventValue> + Send + Sync>),
    Async(Async<P>),
    Around(Arc<dyn Fn(P, Next) -> Task<Result<EventValue>> + Send + Sync>),
}
struct Listener<P> {
    callback: Mutex<Option<Callback<P>>>,
    inner: Weak<Inner<P>>,
    removed: AtomicBool,
    once: bool,
}
struct Inner<P> {
    listeners: Mutex<Vec<Arc<Listener<P>>>>,
}

pub struct Event<P>(Arc<Inner<P>>);
impl<P> Clone for Event<P> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

pub struct Subscription {
    remove: Box<dyn Fn() + Send + Sync>,
    _effect: EffectLease,
}
impl Subscription {
    pub fn remove(&self) {
        (self.remove)();
    }
}

pub struct Next {
    used: AtomicBool,
    callback: Arc<dyn Fn() -> Task<Result<EventValue>> + Send + Sync>,
}
impl Next {
    pub async fn run(&self) -> Result<EventValue> {
        ensure!(!self.used.swap(true, Ordering::AcqRel), "next() called multiple times");
        (self.callback)().await
    }
}

impl<P: Clone + Send + Sync + 'static> Event<P> {
    pub fn new(_name: &str) -> Self {
        Self(Arc::new(Inner { listeners: Mutex::new(Vec::new()) }))
    }
    fn add(&self, scope: &EffectScope, callback: Callback<P>, options: ListenOptions) -> Result<Subscription> {
        let listener = Arc::new(Listener {
            callback: Mutex::new(Some(callback)),
            inner: Arc::downgrade(&self.0),
            removed: AtomicBool::new(false),
            once: options.once,
        });
        let remove = {
            let listener = Arc::downgrade(&listener);
            move || {
                let Some(listener) = listener.upgrade() else {
                    return;
                };
                listener.removed.store(true, Ordering::Release);
                listener.callback.lock().unwrap().take();
                if let Some(inner) = listener.inner.upgrade() {
                    inner.listeners.lock().unwrap().retain(|entry| !Arc::ptr_eq(entry, &listener));
                }
            }
        };
        let effect_remove = remove.clone();
        let effect = scope.own(move || async move {
            effect_remove();
            Ok(())
        })?;
        let mut listeners = self.0.listeners.lock().unwrap();
        ensure!(!scope.is_closing(), "event owner is closing");
        if options.prepend {
            listeners.insert(0, listener);
        } else {
            listeners.push(listener);
        }
        Ok(Subscription { remove: Box::new(remove), _effect: effect })
    }
    pub fn on(
        &self,
        scope: &EffectScope,
        callback: impl Fn(P) -> Result<EventValue> + Send + Sync + 'static,
        options: ListenOptions,
    ) -> Result<Subscription> {
        self.add(scope, Callback::Sync(Arc::new(callback)), options)
    }
    pub fn on_async<F, Fut>(&self, scope: &EffectScope, callback: F) -> Result<Subscription>
    where
        F: Fn(P) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<EventValue>> + Send + 'static,
    {
        self.add(scope, Callback::Async(Arc::new(move |payload| Box::pin(callback(payload)))), ListenOptions::default())
    }
    pub fn around<F, Fut>(&self, scope: &EffectScope, callback: F) -> Result<Subscription>
    where
        F: Fn(P, Next) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<EventValue>> + Send + 'static,
    {
        self.add(
            scope,
            Callback::Around(Arc::new(move |payload, next| Box::pin(callback(payload, next)))),
            ListenOptions::default(),
        )
    }
    fn snapshot(&self) -> Vec<Arc<Listener<P>>> {
        self.0.listeners.lock().unwrap().clone()
    }
    fn admit(listener: &Arc<Listener<P>>) -> Option<Callback<P>> {
        let mut callback = listener.callback.lock().unwrap();
        if listener.once {
            if listener.removed.swap(true, Ordering::AcqRel) {
                return None;
            }
            let admitted = callback.take();
            drop(callback);
            if let Some(inner) = listener.inner.upgrade() {
                inner.listeners.lock().unwrap().retain(|entry| !Arc::ptr_eq(entry, listener));
            }
            admitted
        } else {
            if listener.removed.load(Ordering::Acquire) {
                return None;
            }
            callback.clone()
        }
    }
    pub fn emit(&self, payload: P) -> Result<()> {
        for listener in self.snapshot() {
            if let Some(callback) = Self::admit(&listener) {
                match callback {
                    Callback::Sync(callback) => {
                        callback(payload.clone())?;
                    }
                    _ => return Err(anyhow!("synchronous dispatch requires synchronous listeners")),
                }
            }
        }
        Ok(())
    }
    pub fn bail(&self, payload: P) -> Result<EventValue> {
        for listener in self.snapshot() {
            if let Some(callback) = Self::admit(&listener) {
                let value = match callback {
                    Callback::Sync(callback) => callback(payload.clone())?,
                    _ => return Err(anyhow!("synchronous dispatch requires synchronous listeners")),
                };
                if value.bailed() {
                    return Ok(value);
                }
            }
        }
        Ok(EventValue::Undefined)
    }
    async fn invoke(listener: Arc<Listener<P>>, payload: P) -> Result<EventValue> {
        let Some(callback) = Self::admit(&listener) else {
            return Ok(EventValue::Undefined);
        };
        match callback {
            Callback::Sync(callback) => callback(payload),
            Callback::Async(callback) => callback(payload).await,
            _ => Err(anyhow!("around listeners require waterfall dispatch")),
        }
    }
    pub async fn serial(&self, payload: P) -> Result<EventValue> {
        for listener in self.snapshot() {
            let value = Self::invoke(listener, payload.clone()).await?;
            if value.bailed() {
                return Ok(value);
            }
        }
        Ok(EventValue::Undefined)
    }
    pub async fn parallel(&self, payload: P) -> std::result::Result<(), ParallelErrors> {
        let results =
            join_all(self.snapshot().into_iter().map(|listener| Self::invoke(listener, payload.clone()))).await;
        let errors: Vec<_> =
            results.into_iter().filter_map(|outcome| outcome.err().map(|error| error.to_string())).collect();
        if errors.is_empty() { Ok(()) } else { Err(ParallelErrors { errors }) }
    }
    pub async fn waterfall<F, Fut>(&self, payload: P, tail: F) -> Result<EventValue>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<EventValue>> + Send + 'static,
    {
        Self::dispatch(Arc::new(self.snapshot()), 0, payload, Arc::new(move || Box::pin(tail()))).await
    }
    fn dispatch(
        listeners: Arc<Vec<Arc<Listener<P>>>>,
        mut index: usize,
        payload: P,
        tail: Arc<dyn Fn() -> Task<Result<EventValue>> + Send + Sync>,
    ) -> Task<Result<EventValue>> {
        Box::pin(async move {
            while let Some(listener) = listeners.get(index) {
                index += 1;
                let Some(callback) = Self::admit(listener) else {
                    continue;
                };
                return match callback {
                    Callback::Around(callback) => {
                        let next = Next {
                            used: AtomicBool::new(false),
                            callback: Arc::new({
                                let listeners = listeners.clone();
                                let payload = payload.clone();
                                move || Self::dispatch(listeners.clone(), index, payload.clone(), tail.clone())
                            }),
                        };
                        callback(payload, next).await
                    }
                    Callback::Sync(callback) => callback(payload),
                    Callback::Async(callback) => callback(payload).await,
                };
            }
            tail().await
        })
    }
}
