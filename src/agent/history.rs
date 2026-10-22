use crate::workspace::field;
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};

pub fn tool_calls(message: &Value) -> Result<Vec<Value>> {
    ensure!(message["tool_calls"].is_null() || message["role"] == "assistant", "invalid tool_calls role");
    let calls = message.get("tool_calls").filter(|v| !v.is_null()).cloned().unwrap_or(json!([]));
    let calls = calls.as_array().context("invalid tool_calls")?;
    ensure!(calls.len() <= 128, "too many tool calls");
    for (index, call) in calls.iter().enumerate() {
        let id = field(call, "id")?;
        ensure!(!id.is_empty() && !calls[..index].iter().any(|c| c["id"] == id), "duplicate/empty tool id");
        ensure!(call["type"] == "function", "unsupported tool call type");
        ensure!(!field(&call["function"], "name")?.is_empty(), "empty tool name");
        let args: Value =
            serde_json::from_str(field(&call["function"], "arguments")?).context("invalid tool arguments")?;
        ensure!(args.is_object(), "tool arguments must be an object");
    }
    Ok(calls.clone())
}
pub fn checked_history(value: Value) -> Result<Vec<Value>> {
    let messages: Vec<Value> = serde_json::from_value(value)?;
    ensure!(messages.first().is_some_and(|m| m["role"] == "system"), "missing system message");
    let mut pending = Vec::new();
    for message in &messages {
        if message["role"] == "tool" {
            let id = field(message, "tool_call_id")?;
            let index = pending.iter().position(|p| p == id).context("orphan tool result")?;
            pending.remove(index);
        } else {
            ensure!(pending.is_empty(), "incomplete tool batch");
            ensure!(matches!(message["role"].as_str(), Some("system" | "user" | "assistant")), "invalid role");
            for call in tool_calls(message)? {
                pending.push(field(&call, "id")?.to_owned());
            }
        }
    }
    ensure!(pending.is_empty(), "incomplete tool batch");
    Ok(messages)
}
