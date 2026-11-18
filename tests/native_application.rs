#![cfg(not(target_arch = "wasm32"))]
use anyhow::Result;
use i::application::{AgentApplication, ApplicationHost, Services};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tokio::sync::Semaphore;

struct Provider(&'static str);
struct Workspace;
struct Host {
    requests: Arc<Mutex<Vec<String>>>,
    entered: Option<Arc<Semaphore>>,
    release: Option<Arc<Semaphore>>,
}
impl ApplicationHost<Provider, Workspace> for Host {
    async fn complete(&mut self, services: &Services<Provider, Workspace>, _: &[Value]) -> Result<Value> {
        let requests = self.requests.clone();
        let entered = self.entered.clone();
        let release = self.release.clone();
        services
            .provider()
            .call_async(move |provider, _| async move {
                requests.lock().unwrap().push(provider.0.into());
                if let Some(entered) = entered {
                    entered.add_permits(1);
                }
                if let Some(release) = release {
                    release.acquire().await.unwrap().forget();
                }
                Ok(json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":"done"}}]}))
            })
            .await
    }
    async fn dispatch(&mut self, _: &Services<Provider, Workspace>, _: &Value, _: usize) -> Result<Value> {
        panic!("unexpected tool dispatch")
    }
}
fn history() -> Vec<Value> {
    vec![json!({"role":"system","content":"test"}), json!({"role":"user","content":"keep this conversation"})]
}

#[tokio::test]
async fn provider_replacement_waits_for_checkpoint_and_preserves_its_old_binding_view() {
    struct CheckpointHost {
        entered: Arc<Semaphore>,
        release: Arc<Semaphore>,
        trace: Arc<Mutex<Vec<String>>>,
    }
    impl ApplicationHost<Provider, Workspace> for CheckpointHost {
        async fn before_turn(&mut self, services: &Services<Provider, Workspace>, _: &mut Vec<Value>) -> Result<()> {
            self.entered.add_permits(1);
            self.release.acquire().await.unwrap().forget();
            let name = services.provider().call(|provider, _| provider.0.to_owned())?;
            self.trace.lock().unwrap().push(format!("checkpoint({name})"));
            Ok(())
        }
        async fn complete(&mut self, _: &Services<Provider, Workspace>, _: &[Value]) -> Result<Value> {
            panic!("cancelled task dispatched a model request after checkpoint")
        }
        async fn dispatch(&mut self, _: &Services<Provider, Workspace>, _: &Value, _: usize) -> Result<Value> {
            panic!("unexpected tool dispatch")
        }
    }
    let app = Arc::new(AgentApplication::new(Arc::new(Provider("old")), Arc::new(Workspace)).unwrap());
    let entered = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let trace = Arc::new(Mutex::new(Vec::new()));
    let running = tokio::spawn({
        let app = app.clone();
        let entered = entered.clone();
        let release = release.clone();
        let trace = trace.clone();
        async move {
            let mut conversation = app.attach_conversation(history()).unwrap();
            app.run(&mut CheckpointHost { entered, release, trace }, &mut conversation, 2).await
        }
    });
    entered.acquire().await.unwrap().forget();
    let replacing = app.replace_provider(Arc::new(Provider("new")));
    tokio::pin!(replacing);
    tokio::select! {biased; _ = &mut replacing => panic!("replacement released resources before checkpoint drain"), _ = std::future::ready(()) => {}}
    release.add_permits(1);
    replacing.await.unwrap();
    assert!(running.await.unwrap().is_err());
    assert_eq!(*trace.lock().unwrap(), ["checkpoint(old)"]);
    assert!(app.shutdown().await.closed);
}

#[tokio::test]
async fn ancestor_shutdown_before_run_admission_does_not_orphan_the_context_waiter() {
    let app = AgentApplication::new(Arc::new(Provider("old")), Arc::new(Workspace)).unwrap();
    let mut conversation = app.attach_conversation(history()).unwrap();
    let mut host = Host { requests: Arc::default(), entered: None, release: None };
    let running = app.run(&mut host, &mut conversation, 2);
    tokio::pin!(running);
    tokio::select! {biased; _ = &mut running => panic!("run should wait for admission"), _ = std::future::ready(()) => {}}
    app.context().request_shutdown();
    let result = tokio::time::timeout(std::time::Duration::from_millis(200), running).await;
    assert!(result.is_ok(), "disposed pending run orphaned its context waiter");
    assert!(result.unwrap().is_err());
    assert!(app.shutdown().await.closed);
}

#[tokio::test]
async fn conversation_from_another_application_requires_explicit_attachment() {
    let a = AgentApplication::new(Arc::new(Provider("a")), Arc::new(Workspace)).unwrap();
    let b = AgentApplication::new(Arc::new(Provider("b")), Arc::new(Workspace)).unwrap();
    let mut conversation = a.attach_conversation(history()).unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let mut host = Host { requests: requests.clone(), entered: None, release: None };
    assert!(b.run(&mut host, &mut conversation, 2).await.is_err());
    assert!(requests.lock().unwrap().is_empty());
    b.reattach_conversation(&mut conversation).unwrap();
    b.run(&mut host, &mut conversation, 2).await.unwrap();
    assert_eq!(*requests.lock().unwrap(), ["b"]);
    a.shutdown().await;
    b.shutdown().await;
}

#[tokio::test]
async fn provider_replacement_cancels_and_drains_old_run_without_sending_old_history_to_new_provider() {
    let app = Arc::new(AgentApplication::new(Arc::new(Provider("old")), Arc::new(Workspace)).unwrap());
    let entered = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let conversation = app.attach_conversation(history()).unwrap();
    let running = tokio::spawn({
        let app = app.clone();
        let entered = entered.clone();
        let requests = requests.clone();
        async move {
            let mut conversation = conversation;
            let mut host = Host { requests, entered: Some(entered), release: Some(release) };
            let result = app.run(&mut host, &mut conversation, 2).await;
            (conversation, result)
        }
    });
    entered.acquire().await.unwrap().forget();
    app.replace_provider(Arc::new(Provider("new"))).await.unwrap();
    let (mut conversation, result) = running.await.unwrap();
    assert!(result.is_err());
    assert!(conversation.history().iter().any(|value| value["role"] == "user"));
    let mut host = Host { requests: requests.clone(), entered: None, release: None };
    assert!(app.run(&mut host, &mut conversation, 2).await.unwrap_err().to_string().contains("attach"));
    assert_eq!(*requests.lock().unwrap(), ["old"]);
    app.reattach_conversation(&mut conversation).unwrap();
    app.run(&mut host, &mut conversation, 2).await.unwrap();
    assert_eq!(*requests.lock().unwrap(), ["old", "new"]);
    assert!(app.shutdown().await.closed);
}

#[tokio::test]
async fn shutdown_cancels_pending_request_and_prevents_new_runs() {
    let app = Arc::new(AgentApplication::new(Arc::new(Provider("old")), Arc::new(Workspace)).unwrap());
    let entered = Arc::new(Semaphore::new(0));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let running = tokio::spawn({
        let app = app.clone();
        let entered = entered.clone();
        let requests = requests.clone();
        async move {
            let mut conversation = app.attach_conversation(history()).unwrap();
            app.run(
                &mut Host { requests, entered: Some(entered), release: Some(Arc::new(Semaphore::new(0))) },
                &mut conversation,
                2,
            )
            .await
        }
    });
    entered.acquire().await.unwrap().forget();
    assert!(app.shutdown().await.closed);
    assert!(running.await.unwrap().is_err());
    assert!(app.attach_conversation(history()).is_err());
    assert_eq!(*requests.lock().unwrap(), ["old"]);
}

#[tokio::test]
async fn replacement_cancels_pending_approval_without_dispatch_or_connection_switch() {
    struct ApprovalHost {
        entered: Arc<Semaphore>,
        observed: Arc<Mutex<Vec<String>>>,
    }
    impl ApplicationHost<Provider, Workspace> for ApprovalHost {
        async fn complete(&mut self, _: &Services<Provider, Workspace>, _: &[Value]) -> Result<Value> {
            Ok(json!({"choices":[{"finish_reason":"tool_calls","message":{"role":"assistant","tool_calls":[
                {"id":"write","type":"function","function":{"name":"write","arguments":"{}"}}
            ]}}]}))
        }
        async fn before_dispatch(
            &mut self,
            services: &Services<Provider, Workspace>,
            _: &[Value],
            _: &Value,
        ) -> Result<()> {
            self.entered.add_permits(1);
            services.cancelled().await;
            self.observed.lock().unwrap().push(services.provider().call(|provider, _| provider.0.to_owned())?);
            Ok(())
        }
        async fn dispatch(&mut self, _: &Services<Provider, Workspace>, _: &Value, _: usize) -> Result<Value> {
            panic!("cancelled approval dispatched a tool")
        }
    }
    let app = Arc::new(AgentApplication::new(Arc::new(Provider("old")), Arc::new(Workspace)).unwrap());
    let entered = Arc::new(Semaphore::new(0));
    let observed = Arc::new(Mutex::new(Vec::new()));
    let running = tokio::spawn({
        let (app, entered, observed) = (app.clone(), entered.clone(), observed.clone());
        async move {
            let mut conversation = app.attach_conversation(history()).unwrap();
            let result = app.run(&mut ApprovalHost { entered, observed }, &mut conversation, 2).await;
            (conversation, result)
        }
    });
    entered.acquire().await.unwrap().forget();
    app.replace_provider(Arc::new(Provider("new"))).await.unwrap();
    let (conversation, result) = running.await.unwrap();
    assert!(result.is_err());
    assert_eq!(*observed.lock().unwrap(), ["old"]);
    assert!(
        conversation.history().iter().any(|entry| entry["role"] == "tool" && entry["content"] == "ERROR: cancelled")
    );
    assert!(app.shutdown().await.closed);
}

#[tokio::test]
async fn tool_retirement_waits_for_real_write_and_checkpoint_without_undoing_durable_output() {
    use i::platform::{FileSystem, native::NativeFs};
    use i::workspace::{Limits, Workspace as Files};
    use std::path::Path;
    struct ToolHost {
        entered: Arc<Semaphore>,
        release: Arc<Semaphore>,
        checkpoints: Arc<Mutex<Vec<String>>>,
    }
    impl ApplicationHost<Provider, Files<NativeFs>> for ToolHost {
        async fn complete(&mut self, services: &Services<Provider, Files<NativeFs>>, _: &[Value]) -> Result<Value> {
            assert_eq!(services.provider().call(|provider, _| provider.0)?, "old");
            Ok(json!({"choices":[{"finish_reason":"tool_calls","message":{"role":"assistant","tool_calls":[
                {"id":"write","type":"function","function":{"name":"write","arguments":"{}"}}
            ]}}]}))
        }
        async fn dispatch(
            &mut self,
            services: &Services<Provider, Files<NativeFs>>,
            _: &Value,
            _: usize,
        ) -> Result<Value> {
            let (entered, release) = (self.entered.clone(), self.release.clone());
            services
                .workspace()
                .call_async(|workspace, _| async move {
                    workspace.fs.write_file(Path::new("durable.txt"), b"committed once").await?;
                    entered.add_permits(1);
                    release.acquire().await.unwrap().forget();
                    Ok(json!({"output":"written"}))
                })
                .await
        }
        async fn checkpoint(
            &mut self,
            services: &Services<Provider, Files<NativeFs>>,
            _: &mut Vec<Value>,
        ) -> Result<()> {
            self.checkpoints.lock().unwrap().push(services.provider().call(|provider, _| provider.0.to_owned())?);
            Ok(())
        }
    }
    let root = tempfile::tempdir().unwrap();
    let app = Arc::new(
        AgentApplication::new(
            Arc::new(Provider("old")),
            Arc::new(Files::new(NativeFs::new(root.path().into()).unwrap(), Limits::browser())),
        )
        .unwrap(),
    );
    let entered = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let checkpoints = Arc::new(Mutex::new(Vec::new()));
    let running = tokio::spawn({
        let (app, entered, release, checkpoints) = (app.clone(), entered.clone(), release.clone(), checkpoints.clone());
        async move {
            let mut conversation = app.attach_conversation(history()).unwrap();
            app.run(&mut ToolHost { entered, release, checkpoints }, &mut conversation, 2).await
        }
    });
    entered.acquire().await.unwrap().forget();
    let replacing = app.replace_provider(Arc::new(Provider("new")));
    tokio::pin!(replacing);
    tokio::select! {biased; _ = &mut replacing => panic!("retirement escaped an admitted tool call"), _ = std::future::ready(()) => {}}
    release.add_permits(1);
    replacing.await.unwrap();
    assert!(running.await.unwrap().is_err());
    assert_eq!(*checkpoints.lock().unwrap(), ["old"]);
    assert_eq!(std::fs::read(root.path().join("durable.txt")).unwrap(), b"committed once");
    assert!(app.shutdown().await.closed);
}
