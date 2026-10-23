use super::provider::BrowserConfig;
use crate::{agent::history::tool_calls, workspace::field};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};

fn parts(content: &Value) -> Result<Vec<Value>> {
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
            Some("text") => {
                field(part, "text")?;
                Ok(part.clone())
            }
            Some("image_url") => {
                let url = field(&part["image_url"], "url")?;
                let source = if let Some(data) = url.strip_prefix("data:") {
                    let (mime, data) = data.split_once(";base64,").context("invalid image data URL")?;
                    ensure!(
                        matches!(mime, "image/png" | "image/jpeg" | "image/gif" | "image/webp"),
                        "unsupported image media type"
                    );
                    json!({"type":"base64","media_type":mime,"data":data})
                } else {
                    json!({"type":"url","url":url})
                };
                Ok(json!({"type":"image","source":source}))
            }
            _ => bail!("unsupported Anthropic content part"),
        })
        .collect()
}

pub(super) fn body(config: &BrowserConfig, history: &[Value], definitions: &[Value]) -> Result<Value> {
    let mut system = Vec::new();
    let mut messages: Vec<Value> = Vec::new();
    for message in history {
        let role = field(message, "role")?;
        if role == "system" {
            system.extend(parts(&message["content"])?);
            continue;
        }
        let (role, mut blocks) = match role {
            "tool" => (
                "user",
                vec![
                    json!({"type":"tool_result","tool_use_id":field(message,"tool_call_id")?,"content":parts(&message["content"])?,"is_error":message["content"].as_str().is_some_and(|text| text.starts_with("ERROR:"))}),
                ],
            ),
            "assistant" => {
                let blocks = if let Some(saved) = message.get("_anthropic_content") {
                    let saved = saved.as_array().context("invalid saved Anthropic content")?;
                    // Validate imported opaque blocks against the canonical tool batch before replay.
                    let canonical = tool_calls(message)?;
                    let historical: Vec<_> =
                        canonical.iter().map(|call| json!({"function":{"name":call["function"]["name"]}})).collect();
                    let replay = normalize_response(
                        json!({"type":"message","role":"assistant","content":saved,"stop_reason":if canonical.is_empty() {"end_turn"} else {"tool_use"}}),
                        &historical,
                    )?;
                    ensure!(
                        tool_calls(message)? == tool_calls(&replay["choices"][0]["message"])?,
                        "saved Anthropic tool batch mismatch"
                    );
                    saved.clone()
                } else {
                    let mut blocks = parts(&message["content"])?;
                    for call in tool_calls(message)? {
                        let input: Value = serde_json::from_str(field(&call["function"], "arguments")?)?;
                        blocks.push(
                            json!({"type":"tool_use","id":call["id"],"name":call["function"]["name"],"input":input}),
                        );
                    }
                    blocks
                };
                ("assistant", blocks)
            }
            "user" => ("user", parts(&message["content"])?),
            _ => bail!("unsupported Anthropic message role"),
        };
        if let Some(last) = messages.last_mut().filter(|last| last["role"] == role) {
            last["content"].as_array_mut().context("invalid content")?.append(&mut blocks);
        } else {
            messages.push(json!({"role":role,"content":blocks}));
        }
    }
    let tools: Vec<_> = definitions
        .iter()
        .map(|tool| {
            let f = &tool["function"];
            json!({"name":f["name"],"description":f["description"],"input_schema":f["parameters"]})
        })
        .collect();
    Ok(
        json!({"model":config.model,"system":system,"messages":messages,"tools":tools,"max_tokens":config.max_tokens,"stream":config.stream}),
    )
}

pub(super) fn normalize_response(response: Value, extra: &[Value]) -> Result<Value> {
    ensure!(response["type"] == "message" && response["role"] == "assistant", "invalid Anthropic assistant response");
    let blocks = response["content"].as_array().context("missing Anthropic content")?;
    let reason = match response["stop_reason"].as_str() {
        Some("end_turn" | "stop_sequence") => "stop",
        Some("tool_use") => "tool_calls",
        _ => bail!("incomplete Anthropic response: unsupported stop_reason"),
    };
    let mut text = Vec::new();
    let mut calls = Vec::new();
    for block in blocks {
        match block["type"].as_str() {
            Some("text") => text.push(field(block, "text")?),
            Some("tool_use") => {
                ensure!(block["input"].is_object(), "invalid Anthropic tool input");
                let name = field(block, "name")?;
                ensure!(
                    matches!(name, "read" | "write" | "edit" | "view_image" | "list" | "download" | "glob" | "search")
                        || extra.iter().any(|d| d["function"]["name"] == name),
                    "unsupported tool in browser: {name}"
                );
                calls.push(json!({"id":field(block,"id")?,"type":"function","function":{"name":name,"arguments":block["input"].to_string()}}));
            }
            Some("thinking") => {
                field(block, "thinking")?;
                field(block, "signature")?;
            }
            Some("redacted_thinking") => {
                field(block, "data")?;
            }
            _ => bail!("unsupported Anthropic response block"),
        }
    }
    ensure!((reason == "tool_calls") != calls.is_empty(), "incomplete Anthropic tool batch");
    let message = json!({"role":"assistant","content":text.join("\n"),"tool_calls":calls,"_anthropic_content":blocks});
    tool_calls(&message)?;
    Ok(json!({"choices":[{"index":0,"finish_reason":reason,"message":message}]}))
}
