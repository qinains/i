use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::watch,
    task::JoinHandle,
    time::{Instant, timeout_at},
};

pub(crate) struct Activity {
    heartbeat: watch::Sender<Instant>,
    expired: watch::Receiver<bool>,
    streaming: Arc<AtomicBool>,
    connection: watch::Sender<bool>,
    task: JoinHandle<()>,
    duration: Duration,
}

impl Activity {
    pub fn new(duration: Duration, connection: watch::Sender<bool>) -> Self {
        let (heartbeat, mut updates) = watch::channel(Instant::now() + duration);
        let (expired_tx, expired) = watch::channel(false);
        let streaming = Arc::new(AtomicBool::new(false));
        let flag = streaming.clone();
        let disconnect = connection.clone();
        let task = tokio::spawn(async move {
            loop {
                let deadline = *updates.borrow_and_update();
                match timeout_at(deadline, updates.changed()).await {
                    Ok(Ok(())) => continue,
                    Ok(Err(_)) => break,
                    Err(_) => {
                        let _ = expired_tx.send(true);
                        if flag.load(Ordering::SeqCst) {
                            let _ = disconnect.send(true);
                        }
                        break;
                    }
                }
            }
        });
        Self { heartbeat, expired, streaming, connection, task, duration }
    }

    pub fn touch(&self) {
        let _ = self.heartbeat.send(Instant::now() + self.duration);
    }

    pub fn begin_stream(&self) {
        self.streaming.store(true, Ordering::SeqCst);
        if *self.expired.borrow() {
            let _ = self.connection.send(true);
        }
    }

    pub async fn run<T>(&self, future: impl Future<Output = T>) -> Result<T, u16> {
        let mut expired = self.expired.clone();
        let mut connection = self.connection.subscribe();
        if *expired.borrow() {
            return Err(504);
        }
        if *connection.borrow() {
            return Err(502);
        }
        tokio::select! {
            _ = expired.changed() => Err(504),
            _ = connection.changed() => Err(502),
            result = future => Ok(result),
        }
    }
}

impl Drop for Activity {
    fn drop(&mut self) {
        self.task.abort();
    }
}
