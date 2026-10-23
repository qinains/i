use anyhow::Result;
use i::agent::{self, AgentHost};
use serde_json::{Value, json};
use std::collections::VecDeque;

struct Host {
    replies: VecDeque<Value>,
    effects: usize,
    saved: Vec<Value>,
    cancelled: bool,
    fail_policy: bool,
}
impl AgentHost for Host {
    async fn before_turn(&mut self, _: &mut Vec<Value>) -> Result<()> {
        Ok(())
    }
    async fn complete(&mut self, _: &[Value]) -> Result<Value> {
        Ok(self.replies.pop_front().unwrap())
    }
    async fn before_tools(&mut self, _: &[Value], _: usize) -> Result<usize> {
        Ok(1000)
    }
    async fn dispatch(&mut self, _: &Value, _: usize) -> Result<Value> {
        if self.fail_policy {
            anyhow::bail!("policy worker failed");
        }
        self.effects += 1;
        Ok(json!({"output":"written"}))
    }
    async fn checkpoint(&mut self, h: &mut Vec<Value>) -> Result<()> {
        self.saved = h.clone();
        Ok(())
    }
    async fn finish(&mut self, _: &[Value]) -> Result<()> {
        Ok(())
    }
    fn emit(&mut self, _: Value) -> Result<()> {
        Ok(())
    }
    fn cancelled(&self) -> bool {
        self.cancelled
    }
}
fn call(id: &str) -> Value {
    json!({"id":id,"type":"function","function":{"name":"write","arguments":"{\"path\":\"x\",\"content\":\"y\"}"}})
}
fn response(calls: Value, reason: &str) -> Value {
    json!({"choices":[{"finish_reason":reason,"message":{"role":"assistant","content":null,"reasoning_content":"opaque","tool_calls":calls}}]})
}
fn host(replies: Vec<Value>) -> Host {
    Host { replies: replies.into(), effects: 0, saved: vec![], cancelled: false, fail_policy: false }
}
#[tokio::test]
async fn unlimited_run_continues_beyond_32_rounds_until_completion() {
    let mut replies: Vec<_> = (0..40).map(|n| response(json!([call(&format!("c{n}"))]), "tool_calls")).collect();
    replies.push(response(json!([]), "stop"));
    let mut h = host(replies);
    agent::run_unlimited(&mut h, &mut vec![]).await.unwrap();
    assert_eq!(h.effects, 40);
    assert!(h.replies.is_empty());
}
#[tokio::test]
async fn unlimited_run_still_honors_cancellation() {
    let mut h = host(vec![]);
    h.cancelled = true;
    assert!(agent::run_unlimited(&mut h, &mut vec![]).await.unwrap_err().to_string().contains("cancelled"));
}
#[tokio::test]
async fn malformed_final_call_has_no_side_effects() {
    for calls in [
        json!([call("a"), {"id":"b"}]),
        json!([call("a"), call("a")]),
        json!([call("a"), {"id":"b","type":"function","function":{"name":"write","arguments":"{"}}]),
    ] {
        let mut h = host(vec![response(calls, "tool_calls")]);
        assert!(agent::run(&mut h, &mut vec![], 32).await.is_err());
        assert_eq!(h.effects, 0);
    }
    let mut h = host(vec![response(json!([call("a")]), "length")]);
    assert!(agent::run(&mut h, &mut vec![], 32).await.is_err());
    assert_eq!(h.effects, 0);
}
#[tokio::test]
async fn checkpoint_contains_complete_tool_batch_and_opaque_fields() {
    let mut h = host(vec![response(json!([call("a"), call("b")]), "tool_calls")]);
    let mut history = vec![];
    assert!(agent::run(&mut h, &mut history, 1).await.unwrap_err().to_string().contains("step limit"));
    assert_eq!(h.effects, 2);
    assert_eq!(h.saved[0]["reasoning_content"], "opaque");
    assert_eq!(h.saved[1]["tool_call_id"], "a");
    assert_eq!(h.saved[2]["tool_call_id"], "b");
}
#[tokio::test]
async fn cancellation_prevents_requests_and_effects() {
    let mut h = host(vec![]);
    h.cancelled = true;
    assert!(agent::run(&mut h, &mut vec![], 32).await.is_err());
    assert_eq!(h.effects, 0);
}
#[tokio::test]
async fn policy_failure_is_fatal_and_does_not_continue_the_batch() {
    let mut h = host(vec![response(json!([call("a"), call("b")]), "tool_calls")]);
    h.fail_policy = true;
    let error = agent::run(&mut h, &mut vec![], 1).await.unwrap_err();
    assert!(error.to_string().contains("policy worker failed"));
    assert_eq!(h.effects, 0);
    assert!(h.saved.is_empty());
}
