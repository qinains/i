use anyhow::{Result, bail};
use i::{
    application::{AgentApplication, ApplicationHost, Services, WorkspaceTools},
    browser::provider::BrowserConfig,
    platform::{Capabilities, native::NativeFs},
    workspace::{Limits, Workspace},
};
use serde_json::{Value, json};
use std::sync::Arc;

struct Host {
    journal: Vec<&'static str>,
    reject_dispatch: bool,
}
impl ApplicationHost<BrowserConfig, Workspace<NativeFs>> for Host {
    async fn before_turn(
        &mut self,
        _: &Services<BrowserConfig, Workspace<NativeFs>>,
        _: &mut Vec<Value>,
    ) -> Result<()> {
        Ok(())
    }
    async fn complete(
        &mut self,
        services: &Services<BrowserConfig, Workspace<NativeFs>>,
        history: &[Value],
    ) -> Result<Value> {
        let body = services.provider().call(|provider, _| provider.body(history))??;
        assert!(body["tools"].as_array().unwrap().iter().any(|tool| tool["function"]["name"] == "write"));
        services.provider().call(|provider, _| provider.normalize_response(if history.iter().any(|message| message["role"] == "tool") {
            json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":"done"}}]})
        } else {
            json!({"choices":[{"finish_reason":"tool_calls","message":{"role":"assistant","content":null,"tool_calls":[{"id":"write_1","type":"function","function":{"name":"write","arguments":"{\"path\":\"written.txt\",\"content\":\"real file\"}"}}]}}]})
        }))?
    }
    async fn before_tools(
        &mut self,
        _: &Services<BrowserConfig, Workspace<NativeFs>>,
        _: &[Value],
        _: usize,
    ) -> Result<usize> {
        Ok(1024)
    }
    async fn before_dispatch(
        &mut self,
        _: &Services<BrowserConfig, Workspace<NativeFs>>,
        history: &[Value],
        _: &Value,
    ) -> Result<()> {
        assert_eq!(history.last().unwrap()["role"], "assistant");
        self.journal.push("before");
        if self.reject_dispatch {
            bail!("journal unavailable");
        }
        Ok(())
    }
    async fn dispatch(
        &mut self,
        services: &Services<BrowserConfig, Workspace<NativeFs>>,
        call: &Value,
        _: usize,
    ) -> Result<Value> {
        assert_eq!(self.journal.last(), Some(&"before"));
        self.journal.push("dispatch");
        services
            .workspace()
            .call_async(|workspace, _| async move {
                WorkspaceTools
                    .execute(
                        &workspace,
                        call["function"]["name"].as_str().unwrap(),
                        &serde_json::from_str::<Value>(call["function"]["arguments"].as_str().unwrap())?,
                    )
                    .await
            })
            .await
    }
    async fn after_dispatch(
        &mut self,
        _: &Services<BrowserConfig, Workspace<NativeFs>>,
        _: &[Value],
        _: &Value,
        result: &Value,
    ) -> Result<()> {
        assert!(result["output"].as_str().unwrap().contains("Wrote"));
        self.journal.push("after");
        Ok(())
    }
    async fn checkpoint(&mut self, _: &Services<BrowserConfig, Workspace<NativeFs>>, _: &mut Vec<Value>) -> Result<()> {
        Ok(())
    }
    async fn finish(&mut self, _: &Services<BrowserConfig, Workspace<NativeFs>>, _: &[Value]) -> Result<()> {
        self.journal.push("finish");
        Ok(())
    }
    fn emit(&mut self, _: Value) -> Result<()> {
        Ok(())
    }
    fn cancelled(&self) -> bool {
        false
    }
}

#[tokio::test]
async fn application_uses_actual_services_and_engine_with_durable_dispatch_hooks() {
    let root = tempfile::tempdir().unwrap();
    let provider: Arc<BrowserConfig> =
        Arc::new(serde_json::from_value(json!({"base_url":"https://example.test/v1","model":"test"})).unwrap());
    let workspace = Arc::new(Workspace::new(NativeFs::new(root.path().into()).unwrap(), Limits::browser()));
    let app = AgentApplication::new(provider.clone(), workspace.clone()).unwrap();
    let mut host = Host { journal: vec![], reject_dispatch: false };
    let mut conversation = app
        .attach_conversation(vec![json!({"role":"system","content":"test"}), json!({"role":"user","content":"write"})])
        .unwrap();
    app.run(&mut host, &mut conversation, 2).await.unwrap();
    assert_eq!(std::fs::read_to_string(root.path().join("written.txt")).unwrap(), "real file");
    assert_eq!(host.journal, ["before", "dispatch", "after", "finish"]);
    assert_eq!(conversation.history()[3]["role"], "tool");
    assert_eq!(conversation.history()[4]["content"], "done");
    assert!(app.shutdown().await.closed);
}

#[tokio::test]
async fn closed_application_cannot_execute_again_and_retains_conversation() {
    let root = tempfile::tempdir().unwrap();
    let provider = Arc::new(
        serde_json::from_value::<BrowserConfig>(json!({"base_url":"https://example.test/v1","model":"test"})).unwrap(),
    );
    let workspace = Arc::new(Workspace::new(NativeFs::new(root.path().into()).unwrap(), Limits::browser()));
    let app = AgentApplication::new(provider.clone(), workspace.clone()).unwrap();
    let mut conversation = app
        .attach_conversation(vec![json!({"role":"system","content":"test"}), json!({"role":"user","content":"write"})])
        .unwrap();
    assert!(app.shutdown().await.closed);
    let mut host = Host { journal: vec![], reject_dispatch: false };
    assert!(app.run(&mut host, &mut conversation, 2).await.is_err());
    assert_eq!(conversation.history()[1]["content"], "write");
    assert!(!root.path().join("written.txt").exists());
}

#[test]
fn shared_browser_workspace_tools_do_not_expose_shell() {
    let schemas = WorkspaceTools.schemas(Capabilities::BROWSER);
    assert!(!schemas.as_array().unwrap().iter().any(|tool| tool["function"]["name"] == "shell"));
}

#[tokio::test]
async fn failed_before_dispatch_journal_prevents_side_effects() {
    let root = tempfile::tempdir().unwrap();
    let provider: Arc<BrowserConfig> =
        Arc::new(serde_json::from_value(json!({"base_url":"https://example.test/v1","model":"test"})).unwrap());
    let workspace = Arc::new(Workspace::new(NativeFs::new(root.path().into()).unwrap(), Limits::browser()));
    let app = AgentApplication::new(provider.clone(), workspace.clone()).unwrap();
    let mut host = Host { journal: vec![], reject_dispatch: true };
    let mut conversation = app.attach_conversation(vec![json!({"role":"system","content":"test"})]).unwrap();
    assert!(app.run(&mut host, &mut conversation, 2).await.unwrap_err().to_string().contains("journal unavailable"));
    assert!(!root.path().join("written.txt").exists());
    assert_eq!(host.journal, ["before"]);
    assert!(app.shutdown().await.closed);
}
