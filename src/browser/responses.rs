use super::provider::BrowserConfig;
use crate::{agent::history::tool_calls, workspace::field};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::collections::HashSet;

fn content(value: &Value, assistant: bool) -> Result<Vec<Value>> {
    let kind = "input_text";
    if value.is_null() {
        return Ok(vec![]);
    }
    if let Some(text) = value.as_str() {
        return Ok(vec![json!({"type":kind,"text":text})]);
    }
    value
        .as_array()
        .context("invalid Responses content")?
        .iter()
        .map(|part| match part["type"].as_str() {
            Some("text") => Ok(json!({"type":kind,"text":field(part,"text")?})),
            Some("image_url") if !assistant => {
                Ok(json!({"type":"input_image","image_url":field(&part["image_url"],"url")?}))
            }
            _ => bail!("unsupported Responses input part"),
        })
        .collect()
}

pub(super) fn body(config: &BrowserConfig, history: &[Value], definitions: &[Value]) -> Result<Value> {
    let mut input = Vec::new();
    for message in history {
        match field(message,"role")? {
            "system" | "user" => input.push(json!({"role":message["role"],"content":content(&message["content"],false)?})),
            "tool" => input.push(json!({"type":"function_call_output","call_id":field(message,"tool_call_id")?,"output":content(&message["content"],false)?})),
            "assistant" => {
                if let Some(saved) = message.get("_responses_output") {
                    let calls = tool_calls(message)?;
                    let historical: Vec<_> = calls.iter().map(|call|json!({"function":{"name":call["function"]["name"]}})).collect();
                    let replay = normalize_response(json!({"object":"response","status":"completed","output":saved}),&historical)?;
                    let canonical = &replay["choices"][0]["message"];
                    ensure!(tool_calls(canonical)? == calls && canonical["content"] == message["content"], "saved Responses output mismatch");
                    input.extend(saved.as_array().context("invalid saved Responses output")?.iter().cloned());
                } else {
                    if !message["content"].is_null() { input.push(json!({"role":"assistant","content":content(&message["content"],true)?})); }
                    for call in tool_calls(message)? {
                        input.push(json!({"type":"function_call","call_id":call["id"],"name":call["function"]["name"],"arguments":call["function"]["arguments"]}));
                    }
                }
            }
            _ => bail!("unsupported Responses message role"),
        }
    }
    let tools: Vec<_> = definitions.iter().map(|tool| {
        let function = &tool["function"];
        // Existing file schemas use optional fields; do not silently make them all required.
        json!({"type":"function","name":function["name"],"description":function["description"],"parameters":function["parameters"],"strict":false})
    }).collect();
    let mut body = json!({"model":config.model,"input":input,"tools":tools,"stream":config.stream,"store":false,"include":["reasoning.encrypted_content"],"max_output_tokens":config.max_tokens});
    if config.reasoning_effort != "default" {
        ensure!(config.reasoning_effort != "max", "Responses uses xhigh, not max, for reasoning effort");
        body["reasoning"] = json!({"effort":config.reasoning_effort});
    }
    Ok(body)
}

pub(super) fn normalize_response(response: Value, extra: &[Value]) -> Result<Value> {
    ensure!(response["object"] == "response", "invalid Responses object");
    ensure!(response["status"] == "completed", "Incomplete Responses response: pending tool calls were not executed");
    let output = response["output"].as_array().context("missing Responses output")?;
    ensure!(output.len() <= 1000, "too many Responses output items");
    let mut ids = HashSet::new();
    let mut text = Vec::new();
    let mut calls = Vec::new();
    for item in output {
        let id = field(item, "id")?;
        ensure!(!id.is_empty() && ids.insert(id), "duplicate/empty Responses item id");
        match item["type"].as_str() {
            Some("message") => {
                ensure!(
                    item["role"] == "assistant" && item["status"] == "completed",
                    "invalid Responses assistant message"
                );
                for part in item["content"].as_array().context("invalid Responses message content")? {
                    text.push(match part["type"].as_str() {
                        Some("output_text") => field(part, "text")?,
                        Some("refusal") => field(part, "refusal")?,
                        _ => bail!("unsupported Responses output content"),
                    });
                }
            }
            Some("function_call") => {
                ensure!(item["status"] == "completed", "incomplete Responses function call");
                let name = field(item, "name")?;
                ensure!(
                    matches!(name, "read" | "write" | "edit" | "view_image" | "list" | "download" | "glob" | "search")
                        || extra.iter().any(|tool| tool["function"]["name"] == name),
                    "unsupported tool in browser: {name}"
                );
                calls.push(json!({"id":field(item,"call_id")?,"type":"function","function":{"name":name,"arguments":field(item,"arguments")?}}));
            }
            Some("reasoning") => {
                if let Some(encrypted) = item.get("encrypted_content").filter(|value| !value.is_null()) {
                    ensure!(encrypted.is_string(), "invalid encrypted reasoning");
                }
                for summary in item["summary"].as_array().context("invalid reasoning summary")? {
                    ensure!(summary["type"] == "summary_text", "unsupported reasoning summary");
                    field(summary, "text")?;
                }
                ensure!(item["status"].is_null() || item["status"] == "completed", "incomplete reasoning item");
            }
            _ => bail!("unsupported Responses output item"),
        }
    }
    let message = json!({"role":"assistant","content":text.join("\n"),"tool_calls":calls,"_responses_output":output});
    tool_calls(&message)?;
    Ok(
        json!({"choices":[{"index":0,"finish_reason":if calls.is_empty(){"stop"}else{"tool_calls"},"message":message}],"usage":response["usage"]}),
    )
}
