use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Notify;

#[derive(Default)]
struct Signal {
    cancelled: AtomicBool,
    changed: Notify,
}

#[derive(Clone, Default)]
pub struct Cancellation(Arc<Signal>);

impl Cancellation {
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }
    pub(crate) fn cancel(&self) -> bool {
        let first = !self.0.cancelled.swap(true, Ordering::AcqRel);
        self.0.changed.notify_waiters();
        first
    }
    pub async fn cancelled(&self) {
        loop {
            let changed = self.0.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.is_cancelled() {
                return;
            }
            changed.await;
        }
    }
}
