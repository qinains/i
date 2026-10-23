use super::{Protocol, field};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};

pub(super) fn endpoint(base: &str, protocol: Protocol) -> String {
    let base = base.trim_end_matches('/');
    match protocol {
        Protocol::Openai => format!("{base}/chat/completions"),
        Protocol::Anthropic if base.ends_with("/v1") => format!("{base}/messages"),
        Protocol::Anthropic => format!("{base}/v1/messages"),
    }
}
fn anthropic_parts(content: &Value) -> Result<Vec<Value>> {
    if let Some(text) = content.as_str() {
        return Ok(if text.is_empty() { vec![] } else { vec![json!({"type":"text","text":text})] });
    }
    if content.is_null() {
        return Ok(vec![]);
    }
    content
        .as_array()
        .context("invalid message content")?
        .iter()
        .map(|part| match part["type"].as_str() {
            Some("text") => Ok(part.clone()),
            Some("file") => Ok(json!({"type":"image","source":{"type":"file","file_id":field(part,"file_id")?}})),
            Some("image_url") => {
                let url = field(&part["image_url"], "url")?;
                let source = if let Some(data) = url.strip_prefix("data:") {
                    let (mime, data) = data.split_once(";base64,").context("invalid image data URL")?;
                    json!({"type":"base64","media_type":mime,"data":data})
                } else {
                    json!({"type":"url","url":url})
                };
                Ok(json!({"type":"image","source":source}))
            }
            _ => bail!("unsupported content part"),
        })
        .collect()
}
pub(super) fn request_body(
    protocol: Protocol,
    model: &str,
    history: &[Value],
    max_tokens: u64,
    definitions: &[Value],
) -> Result<Value> {
    let tools: Vec<_> = definitions.iter().map(|f| tool_schema(protocol, f)).collect();
    if protocol == Protocol::Openai {
        let mut history = history.to_vec();
        for message in &mut history {
            if let Some(object) = message.as_object_mut() {
                object.retain(|key, _| !key.starts_with('_'));
            }
        }
        return Ok(json!({"model":model,"messages":history,"tools":tools,
            "tool_choice":"auto","max_completion_tokens":max_tokens}));
    }
    let mut system = Vec::new();
    let mut messages: Vec<Value> = Vec::new();
    for message in history {
        let role = field(message, "role")?;
        if role == "system" {
            system.extend(anthropic_parts(&message["content"])?);
            continue;
        }
        let (role, mut blocks) = if role == "tool" {
            (
                "user",
                vec![json!({"type":"tool_result","tool_use_id":message["tool_call_id"],
                "content":message["content"],"is_error":message["content"].as_str().is_some_and(|s| s.starts_with("ERROR:"))})],
            )
        } else if role == "assistant" {
            if let Some(saved) = message.get("_anthropic_content") {
                (role, saved.as_array().context("invalid saved Anthropic content")?.clone())
            } else {
                let mut blocks = anthropic_parts(&message["content"])?;
                let calls = message.get("tool_calls").filter(|v| !v.is_null()).cloned().unwrap_or(json!([]));
                for call in calls.as_array().context("invalid tool calls")? {
                    let input: Value = serde_json::from_str(field(&call["function"], "arguments")?)?;
                    blocks
                        .push(json!({"type":"tool_use","id":call["id"],"name":call["function"]["name"],"input":input}));
                }
                (role, blocks)
            }
        } else {
            ensure!(role == "user", "unsupported message role: {role}");
            (role, anthropic_parts(&message["content"])?)
        };
        if let Some(last) = messages.last_mut().filter(|last| last["role"] == role) {
            last["content"].as_array_mut().context("invalid message blocks")?.append(&mut blocks);
        } else {
            messages.push(json!({"role":role,"content":blocks}));
        }
    }
    Ok(json!({"model":model,"system":system,"messages":messages,"tools":tools,
        "tool_choice":{"type":"auto"},"max_tokens":max_tokens}))
}
pub(super) fn normalize_response(protocol: Protocol, response: Value) -> Result<Value> {
    if protocol == Protocol::Openai {
        return Ok(response);
    }
    ensure!(
        response["type"] == "message" && response["role"] == "assistant",
        "invalid Anthropic response: expected assistant message"
    );
    let blocks = response["content"].as_array().context("missing Anthropic content")?;
    let mut text = Vec::new();
    let mut calls = Vec::new();
    for block in blocks {
        match block["type"].as_str() {
            Some("text") => text.push(field(block, "text")?),
            Some("tool_use") => {
                ensure!(block["input"].is_object(), "invalid tool_use input");
                calls.push(json!({"id":field(block,"id")?,"type":"function","function":{
                    "name":field(block,"name")?,"arguments":block["input"].to_string()}}));
            }
            Some("thinking" | "redacted_thinking") => {}
            _ => bail!("unsupported Anthropic response block: {}", block["type"]),
        }
    }
    let reason = match response["stop_reason"].as_str() {
        Some("end_turn" | "stop_sequence") => "stop",
        Some("tool_use") => "tool_calls",
        Some(other) => other,
        None => "unknown",
    };
    ensure!(calls.is_empty() || reason == "tool_calls", "incomplete Anthropic tool response");
    Ok(json!({"choices":[{"finish_reason":reason,"message":{"role":"assistant",
        "content":text.join("\n"),"tool_calls":calls,"_anthropic_content":blocks}}]}))
}
fn tool_schema(protocol: Protocol, f: &Value) -> Value {
    let definition = json!({"name":f["name"],"description":f["description"],"parameters":f["parameters"]});
    match protocol {
        Protocol::Openai => json!({"type":"function","function":definition}),
        Protocol::Anthropic => json!({"name":f["name"],"description":f["description"],"input_schema":f["parameters"]}),
    }
}
