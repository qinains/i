//! Background installation progress and a cancellation/commit boundary.
use anyhow::{Context as _, Result, ensure};
use std::sync::{Arc, Mutex, mpsc};

pub(super) enum Event {
    Progress(String),
    Finished(Result<()>),
}
#[derive(Clone)]
pub(super) struct Context {
    pub proxy: super::proxy::Setting,
    state: Arc<Mutex<State>>,
    sender: mpsc::Sender<Event>,
}
#[derive(Default)]
struct State {
    cancelled: bool,
    committed: bool,
}
impl Context {
    pub fn check(&self) -> Result<()> {
        ensure!(
            !self.state.lock().map_err(|_| anyhow::anyhow!("Installation state unavailable"))?.cancelled,
            "Plugin installation cancelled"
        );
        Ok(())
    }
    pub fn progress(&self, message: impl Into<String>) -> Result<()> {
        self.check()?;
        let _ = self.sender.send(Event::Progress(message.into()));
        Ok(())
    }
    pub fn commit(&self, write: impl FnOnce() -> Result<()>) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| anyhow::anyhow!("Installation state unavailable"))?;
        ensure!(!state.cancelled, "Plugin installation cancelled");
        write()?;
        state.committed = true;
        Ok(())
    }
}
pub(super) struct Job {
    pub id: String,
    context: Context,
    receiver: mpsc::Receiver<Event>,
}
impl Job {
    pub fn start(id: String, work: impl FnOnce(Context) -> Result<()> + Send + 'static) -> Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let context = Context {
            state: Arc::new(Mutex::new(State::default())),
            sender,
            proxy: super::proxy::snapshot("plugins")?,
        };
        let child = context.clone();
        std::thread::Builder::new()
            .name("plugin-installer".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(child.clone())))
                    .unwrap_or_else(|_| Err(anyhow::anyhow!("Plugin installer stopped unexpectedly")));
                let _ = child.sender.send(Event::Finished(result));
            })
            .context("Cannot start background installer")?;
        Ok(Self { id, context, receiver })
    }
    pub fn poll(&self) -> Option<Event> {
        self.receiver.try_recv().ok()
    }
    pub fn cancel(&self) -> bool {
        let mut state = self.context.state.lock().unwrap_or_else(|e| e.into_inner());
        state.cancelled = true;
        !state.committed
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_never_publishes_a_late_download() {
        let (entered, waiting) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let written = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = written.clone();
        let job = Job::start("chatgpt".into(), move |context| {
            context.progress("Downloading 0 KiB")?;
            entered.send(()).unwrap();
            gate.recv().unwrap();
            context.commit(|| {
                flag.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            })
        })
        .unwrap();
        waiting.recv_timeout(std::time::Duration::from_secs(2)).unwrap();
        assert!(matches!(job.poll(), Some(Event::Progress(_))));
        assert!(job.cancel());
        release.send(()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if let Some(Event::Finished(result)) = job.poll() {
                assert!(result.is_err());
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(!written.load(std::sync::atomic::Ordering::SeqCst));
    }
    #[test]
    fn cancellation_after_commit_does_not_claim_the_install_was_cancelled() {
        let (sender, receiver) = mpsc::channel();
        let context =
            Context { state: Arc::new(Mutex::new(State::default())), sender, proxy: crate::proxy::Setting::Inherit };
        context.commit(|| Ok(())).unwrap();
        let job = Job { id: "chatgpt".into(), context, receiver };
        assert!(!job.cancel());
    }
}
