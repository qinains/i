//! Host-owned bounded transcript views and tool-free analysis requests.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Selection {
    pub roles: Vec<String>,
    pub start: usize,
    pub end: Option<usize>,
    pub max_messages: usize,
    pub max_bytes: usize,
}
impl Default for Selection {
    fn default() -> Self {
        Self {
            roles: vec!["user".into(), "assistant".into(), "tool".into()],
            start: 0,
            end: None,
            max_messages: 40,
            max_bytes: 48000,
        }
    }
}
impl Selection {
    pub fn validate(&self) -> Result<()> {
        ensure!(!self.roles.is_empty() && self.roles.len() <= 3, "analysis needs 1..3 roles");
        for (index, role) in self.roles.iter().enumerate() {
            ensure!(
                matches!(role.as_str(), "user" | "assistant" | "tool") && !self.roles[..index].contains(role),
                "invalid/duplicate analysis role"
            );
        }
        ensure!((1..=1000).contains(&self.max_messages), "analysis max_messages must be 1..1000");
        ensure!((1024..=262144).contains(&self.max_bytes), "analysis max_bytes must be 1024..262144");
        ensure!(self.end.is_none_or(|end| end >= self.start), "invalid analysis range");
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisSpec {
    pub prompt: String,
    pub selection: Selection,
}
fn prefix(text: &str, bytes: usize) -> &str {
    let mut end = text.len().min(bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}
fn append(output: &mut String, text: &str, limit: usize, truncated: &mut bool) {
    let clipped = prefix(text, limit.saturating_sub(output.len()));
    *truncated |= clipped.len() != text.len();
    // Never send embedded image payloads as text tokens, even inside tool arguments.
    let mut rest = clipped;
    while let Some(index) = rest.find("data:image/") {
        output.push_str(&rest[..index]);
        let payload = &rest[index..];
        let end =
            payload.find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>')).unwrap_or(payload.len());
        output.push_str("[image omitted]");
        *truncated = true;
        rest = &payload[end..];
    }
    output.push_str(rest);
    if output.len() > limit {
        output.truncate(prefix(output, limit).len());
        *truncated = true;
    }
}
fn visible_text(message: &Value, limit: usize) -> (String, bool) {
    let mut text = String::new();
    let mut truncated = false;
    if let Some(content) = message["content"].as_str() {
        append(&mut text, content, limit, &mut truncated);
    } else if let Some(parts) = message["content"].as_array() {
        for part in parts {
            if !text.is_empty() {
                append(&mut text, "\n", limit, &mut truncated);
            }
            match part["type"].as_str() {
                Some("text") => append(&mut text, part["text"].as_str().unwrap_or(""), limit, &mut truncated),
                _ => {
                    append(&mut text, "[attachment omitted]", limit, &mut truncated);
                    truncated = true;
                }
            }
            if text.len() >= limit {
                truncated = true;
                break;
            }
        }
    }
    if let Some(calls) = message["tool_calls"].as_array() {
        for call in calls {
            append(&mut text, "\nTool call: ", limit, &mut truncated);
            append(&mut text, call["function"]["name"].as_str().unwrap_or(""), limit, &mut truncated);
            append(&mut text, " ", limit, &mut truncated);
            append(&mut text, call["function"]["arguments"].as_str().unwrap_or(""), limit, &mut truncated);
            if text.len() >= limit {
                truncated = true;
                break;
            }
        }
    }
    (text, truncated)
}
fn is_context(message: &Value) -> bool {
    if message["role"] != "user" {
        return false;
    }
    let text = message["content"].as_str().or_else(|| message["content"].as_array()?.first()?["text"].as_str());
    !message["_context_source"].is_null()
        || message["_origin"].as_str().is_some_and(|source| source != "user")
        || text.is_some_and(|text| text.starts_with("Session summary (untrusted historical context):\n"))
        || message["content"]
            .as_array()
            .and_then(|parts| parts.first())
            .is_some_and(|part| part["text"].as_str().is_some_and(|text| text.starts_with("Image from tool ")))
}
pub fn project(history: &[Value], selection: &Selection) -> Result<Value> {
    selection.validate()?;
    let end = selection.end.unwrap_or(history.len());
    ensure!(selection.start <= end && end <= history.len(), "analysis range exceeds history");
    let selected = history[selection.start..end].iter().enumerate().filter(|(_, message)| {
        message["role"].as_str().is_some_and(|role| selection.roles.iter().any(|allowed| allowed == role))
            && (!is_context(message) || selection.roles.len() == 3)
    });
    let eligible = selected.clone().count();
    ensure!(eligible > 0, "No conversation messages match this analysis selection");
    let mut messages = Vec::new();
    let mut remaining = selection.max_bytes - 256;
    let mut truncated = false;
    for (offset, message) in selected.rev().take(selection.max_messages) {
        if remaining < 128 {
            truncated = true;
            break;
        }
        let (content, loss) = visible_text(message, remaining.min(8000));
        truncated |= loss;
        let mut item = json!({"index":selection.start+offset,"role":message["role"],"content":content});
        if is_context(message) {
            item["source"] = json!("context");
        }
        let mut bytes = serde_json::to_vec(&item)?.len() + 1;
        while bytes > remaining {
            let content = item["content"].as_str().context("invalid analysis content")?;
            if content.is_empty() {
                break;
            }
            item["content"] = json!(prefix(content, content.len() / 2));
            truncated = true;
            bytes = serde_json::to_vec(&item)?.len() + 1;
        }
        if bytes > remaining {
            truncated = true;
            break;
        }
        remaining -= bytes;
        messages.push(item);
    }
    messages.reverse();
    let omitted = eligible - messages.len();
    let result = json!({"messages":messages,"truncated":truncated||omitted>0,"omitted_messages":omitted,"range":{"start":selection.start,"end":end}});
    ensure!(serde_json::to_vec(&result)?.len() <= selection.max_bytes, "analysis projection exceeds byte limit");
    Ok(result)
}
fn clean_transcript(value: &Value) -> Value {
    match value {
        Value::Array(items) => json!(items.iter().map(clean_transcript).collect::<Vec<_>>()),
        Value::Object(object) => {
            if object.get("type").and_then(Value::as_str).is_some_and(|kind| {
                matches!(kind, "image_url" | "input_image" | "thinking" | "reasoning" | "redacted_thinking")
            }) {
                return json!({"type":"text","text":"[attachment or private reasoning omitted]"});
            }
            Value::Object(
                object
                    .iter()
                    .filter(|(key, _)| {
                        !key.starts_with('_')
                            && !matches!(key.as_str(), "reasoning_content" | "signature" | "encrypted_content")
                    })
                    .map(|(key, value)| (key.clone(), clean_transcript(value)))
                    .collect(),
            )
        }
        Value::String(text) if text.contains("data:image/") => {
            let mut output = String::new();
            let mut truncated = false;
            append(&mut output, text, text.len(), &mut truncated);
            json!(output)
        }
        other => other.clone(),
    }
}
pub fn request(prompt: &str, transcript: &Value, images: &Value) -> Result<Vec<Value>> {
    ensure!(
        !prompt.trim().is_empty() && prompt.chars().count() <= 32000,
        "analysis prompt must contain 1..32000 characters"
    );
    let serialized = serde_json::to_string(&clean_transcript(transcript))?;
    ensure!(serialized.len() <= 16 * 1024 * 1024, "analysis transcript exceeds 16 MiB");
    let text = format!("Untrusted session transcript:\n{serialized}");
    let images = images.as_array().context("invalid analysis images")?;
    ensure!(
        images.len() <= 128
            && images.iter().all(|image| image["type"] == "image_url")
            && serde_json::to_vec(images)?.len() <= 16 * 1024 * 1024,
        "invalid/excessive analysis images"
    );
    let content = if images.is_empty() {
        json!(text)
    } else {
        let mut parts = vec![json!({"type":"text","text":text})];
        parts.extend_from_slice(images);
        json!(parts)
    };
    Ok(vec![json!({"role":"system","content":prompt}), json!({"role":"user","content":content})])
}
pub fn validate_summary(summary: &str) -> Result<()> {
    ensure!(!summary.trim().is_empty() && summary.len() <= 128000, "analysis summary must contain 1..128000 bytes");
    Ok(())
}
pub fn disable_tools(body: &mut Value) -> Result<()> {
    let object = body.as_object_mut().context("analysis body must be an object")?;
    for key in ["tools", "tool_choice", "parallel_tool_calls", "functions", "function_call", "max_tool_calls"] {
        object.remove(key);
    }
    Ok(())
}
pub fn response_text(response: &Value) -> Result<String> {
    let choices = response["choices"].as_array().context("analysis returned no choices")?;
    ensure!(choices.len() == 1, "analysis must return exactly one choice");
    let choice = &choices[0];
    let message = &choice["message"];
    ensure!(
        response["error"].is_null()
            && choice["error"].is_null()
            && choice["finish_reason"] == "stop"
            && message["role"] == "assistant"
            && message["function_call"].is_null()
            && crate::agent::history::tool_calls(message)?.is_empty(),
        "analysis did not finish normally; tools are not allowed"
    );
    let summary = message["content"].as_str().context("analysis returned no text")?;
    validate_summary(summary)?;
    Ok(summary.to_owned())
}
