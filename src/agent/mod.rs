pub mod history;
use crate::workspace::{
    field,
    text::{clip, image_mime},
};
use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};

pub trait AgentHost {
    async fn before_turn(&mut self, history: &mut Vec<Value>) -> Result<()>;
    async fn complete(&mut self, history: &[Value]) -> Result<Value>;
    async fn before_tools(&mut self, history: &[Value], pending: usize) -> Result<usize>;
    async fn dispatch(&mut self, call: &Value, image_slots: usize) -> Result<Value>;
    async fn checkpoint(&mut self, history: &mut Vec<Value>) -> Result<()>;
    async fn finish(&mut self, history: &[Value]) -> Result<()>;
    fn emit(&mut self, event: Value) -> Result<()>;
    fn cancelled(&self) -> bool;
}

pub async fn run<H: AgentHost>(host: &mut H, messages: &mut Vec<Value>, max_steps: usize) -> Result<()> {
    ensure!(max_steps > 0, "max_steps must be positive");
    run_with_limit(host, messages, Some(max_steps)).await
}

pub async fn run_unlimited<H: AgentHost>(host: &mut H, messages: &mut Vec<Value>) -> Result<()> {
    run_with_limit(host, messages, None).await
}

async fn run_with_limit<H: AgentHost>(host: &mut H, messages: &mut Vec<Value>, max_steps: Option<usize>) -> Result<()> {
    let mut step = 0usize;
    loop {
        step = step.saturating_add(1);
        if max_steps.is_some_and(|limit| step > limit) {
            bail!("step limit exceeded");
        }
        ensure!(!host.cancelled(), "cancelled");
        host.emit(json!({"type":"step","step":step,"max_steps":max_steps}))?;
        host.before_turn(messages).await?;
        ensure!(!host.cancelled(), "cancelled");
        let response = host.complete(messages).await?;
        ensure!(!host.cancelled(), "cancelled");
        let mut message = response["choices"][0]["message"].clone();
        ensure!(message.is_object(), "API response has no assistant message");
        message["role"] = json!("assistant");
        let calls = history::tool_calls(&message)?;
        let reason = &response["choices"][0]["finish_reason"];
        ensure!(calls.is_empty() || reason == "tool_calls", "incomplete tool response");
        if let Some(content) = message["content"].as_str() {
            host.emit(json!({"type":"assistant","text":content}))?;
        }
        messages.push(message);
        if calls.is_empty() {
            host.checkpoint(messages).await?;
            ensure!(reason == "stop", "model stopped: {}", reason);
            host.finish(messages).await?;
            return Ok(());
        }
        let limit = host.before_tools(messages, calls.len()).await?;
        let mut images = Vec::new();
        for call in calls {
            let name = field(&call["function"], "name")?;
            host.emit(json!({"type":"tool","name":name,"arguments":call["function"]["arguments"]}))?;
            // Hosts encode ordinary tool failures; hook/transport policy failures remain fatal.
            let result = if host.cancelled() {
                json!({"output":"ERROR: cancelled"})
            } else {
                host.dispatch(&call, 8 - images.len() / 2).await?
            };
            for image in result.get("images").unwrap_or(&json!([])).as_array().context("invalid tool images")? {
                ensure!(images.len() / 2 < 8 && image["type"] == "image_url", "invalid/excessive tool images");
                let url = field(&image["image_url"], "url")?;
                let (header, encoded) = url.split_once(";base64,").context("tool images must be data URLs")?;
                let bytes = STANDARD.decode(encoded)?;
                ensure!(
                    bytes.len() <= 8 * 1024 * 1024 && header == format!("data:{}", image_mime(&bytes)?),
                    "invalid image"
                );
                images.push(json!({"type":"text","text":format!("Image from tool {}",call["id"])}));
                images.push(image.clone());
            }
            let output = field(&result, "output")?;
            host.emit(json!({"type":"tool_result","name":name,"text":clip(output,600)}))?;
            messages.push(json!({"role":"tool","tool_call_id":call["id"],"content":clip(output,limit)}));
        }
        if !images.is_empty() {
            messages.push(json!({"role":"user","content":images}));
        }
        host.checkpoint(messages).await?;
        ensure!(!host.cancelled(), "cancelled");
    }
}
