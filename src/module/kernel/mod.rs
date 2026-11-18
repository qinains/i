//! Native composition kernel. Browser composition is owned by upstream Cordis JS.
mod binding;
mod cancellation;
mod component;
mod context;
mod effect;
mod events;
mod fiber;
mod service;
mod shutdown;

pub use binding::BindingLease;
pub use cancellation::Cancellation;
pub use component::{Component, ComponentId};
pub use context::{ActivationContext, Context, RealmId};
pub use effect::{EffectLease, EffectScope};
pub use events::{Event, EventValue, ListenOptions, Next, ParallelErrors, Subscription};
pub use fiber::{Fiber, FiberId, FiberState};
pub use service::{ServiceAccess, ServiceBorrow, ServiceCall, ServiceKey};
pub use shutdown::ShutdownReport;

use std::future::Future;
use std::pin::Pin;
pub(crate) type Task<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub(crate) fn next_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}
