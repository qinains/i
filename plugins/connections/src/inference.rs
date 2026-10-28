//! Native-only Responses transport. Credentials never enter plugin JavaScript.
use crate::Auth;
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::{collections::HashSet, time::Duration};

const FAILURE: &str = "ChatGPT response could not be completed";
const MAX_REQUEST: usize = 16 * 1024 * 1024;
const MAX_STREAM: usize = 48 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(1800);

#[derive(Debug)]
pub struct SafeDiagnostic(String);
impl SafeDiagnostic {
    pub fn authentication_http(stage: &'static str, status: u16, hint: &'static str) -> Self {
        Self(format!("ChatGPT {stage} failed (HTTP {status}): {hint}"))
    }
    pub fn stage(stage: &'static str, hint: &'static str) -> Self {
        Self(format!("ChatGPT {stage} failed: {hint}"))
    }
}
impl std::fmt::Display for SafeDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for SafeDiagnostic {}
pub fn worker_diagnostic(error: &anyhow::Error) -> String {
    error
        .chain()
        .find_map(|error| error.downcast_ref::<SafeDiagnostic>())
        .map(ToString::to_string)
        .unwrap_or_else(|| "Connection operation failed; check login and model selection".into())
}
fn http_diagnostic(status: u16, request_id: Option<&str>, body: Option<&Value>) -> SafeDiagnostic {
    const CODES: &[&str] = &[
        "subscription_sharing_user_not_eligible",
        "subscription_sharing_usage_limit_exceeded",
        "subscription_sharing_usage_unavailable",
        "subscription_sharing_unsupported_capability",
        "subscription_sharing_route_not_supported",
        "subscription_sharing_invalid_user",
        "subscription_sharing_user_unavailable",
        "chatpass_v2_scope_not_authorized",
        "chatpass_v2_invalid_authorization_context",
        "model_not_found",
        "invalid_request_error",
        "insufficient_quota",
        "rate_limit_exceeded",
        "server_error",
        "invalid_api_key",
        "unsupported_parameter",
        "unsupported_value",
        "context_length_exceeded",
    ];
    const PARAMS: &[&str] = &[
        "model",
        "input",
        "tools",
        "tool_choice",
        "reasoning",
        "reasoning.effort",
        "reasoning.mode",
        "text",
        "store",
        "stream",
        "service_tier",
    ];
    let shape = if body.is_some_and(|body| body.get("error").is_some()) {
        "error"
    } else if body.is_some_and(|body| body.get("detail").is_some()) {
        "detail"
    } else {
        "unrecognized"
    };
    let code = body
        .and_then(|body| body.pointer("/error/code"))
        .and_then(Value::as_str)
        .filter(|code| CODES.contains(code))
        .unwrap_or("unrecognized");
    let param = body
        .and_then(|body| body.pointer("/error/param"))
        .and_then(Value::as_str)
        .filter(|param| PARAMS.contains(param))
        .unwrap_or("unrecognized");
    let request_id = request_id
        .filter(|id| {
            id.starts_with("req_")
                && id.len() <= 128
                && id.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        })
        .unwrap_or("unavailable");
    let hint = match status {
        400 | 422 => "request/model/tool or thinking options were rejected; check /model settings; do not replay the same invalid request",
        401 => "selected account or signed ChatGPT plan permission was not accepted; check login/scopes",
        403 => "account/workspace/region or direct-route permission was denied; repeated login may not resolve policy restrictions",
        404 => "model or endpoint unavailable for this account; check /model",
        429 => "usage or rate limit reached; check ChatGPT usage and wait before retrying",
        500..=599 => "provider/direct-route service unavailable; credentials were preserved; try later",
        _ => "unexpected provider response; check connection settings",
    };
    SafeDiagnostic(format!("ChatGPT response request failed (HTTP {status}; body={shape}; code={code}; param={param}; request_id={request_id}): {hint}"))
}
async fn rejected_response(mut response: reqwest::Response) -> SafeDiagnostic {
    let status = response.status().as_u16();
    let request_id = response.headers().get("x-request-id").and_then(|value| value.to_str().ok()).map(str::to_owned);
    let body = tokio::time::timeout(Duration::from_secs(5), async {
        let mut bytes = Vec::new();
        while let Ok(Some(chunk)) = response.chunk().await {
            if bytes.len().saturating_add(chunk.len()) > 16_384 {
                return None;
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice::<Value>(&bytes).ok()
    })
    .await
    .ok()
    .flatten();
    http_diagnostic(status, request_id.as_deref(), body.as_ref())
}

pub fn prepare(mut body: Value) -> Result<Value> {
    if serde_json::to_vec(&body).map_err(|_| anyhow!(FAILURE))?.len() > MAX_REQUEST {
        bail!("ChatGPT request is too large");
    }
    let object = body.as_object_mut().ok_or_else(|| anyhow!("ChatGPT request must be an object"))?;
    let model = object.get("model").and_then(Value::as_str).ok_or_else(|| anyhow!("Choose a ChatGPT model"))?;
    if model.trim().is_empty() || model.len() > 256 {
        bail!("Invalid ChatGPT model");
    }
    for field in [
        "background",
        "conversation",
        "max_output_tokens",
        "max_tool_calls",
        "metadata",
        "moderation",
        "multi_agent",
        "prompt",
        "prompt_cache_retention",
        "safety_identifier",
        "temperature",
        "top_logprobs",
        "top_p",
        "truncation",
        "user",
        "previous_response_id",
    ] {
        object.remove(field);
    }
    object.insert("store".into(), Value::Bool(false));
    object.insert("stream".into(), Value::Bool(true));
    let input = object
        .get_mut("input")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow!("ChatGPT input must be an array"))?;
    if input.len() > 4096 {
        bail!("ChatGPT input has too many items");
    }
    for item in input {
        let item = item.as_object_mut().ok_or_else(|| anyhow!("Invalid ChatGPT input item"))?;
        if item.get("role").and_then(Value::as_str) == Some("system") {
            item.insert("role".into(), Value::String("developer".into()));
        }
        match item.get("type").and_then(Value::as_str) {
            Some("function_call") => {
                if item.get("namespace").is_some_and(|n| n != "i") {
                    bail!("Unsupported ChatGPT tool namespace");
                }
                item.insert("namespace".into(), Value::String("i".into()));
            }
            Some("additional_tools" | "input_audio" | "input_video") => bail!("Unsupported ChatGPT input type"),
            _ => {}
        }
        if let Some(content) = item.get("content").and_then(Value::as_array) {
            for part in content {
                if part
                    .get("type")
                    .and_then(Value::as_str)
                    .is_some_and(|t| matches!(t, "input_audio" | "input_video" | "audio" | "video"))
                {
                    bail!("Unsupported ChatGPT input type");
                }
            }
        }
    }
    let mut names = HashSet::new();
    if let Some(raw) = object.remove("tools") {
        let tools = raw.as_array().ok_or_else(|| anyhow!("Invalid ChatGPT tools"))?;
        let mut functions = vec![];
        for tool in tools {
            if tool.get("type").and_then(Value::as_str) == Some("namespace") {
                if tool.get("name").and_then(Value::as_str) != Some("i") {
                    bail!("Unsupported ChatGPT tool namespace");
                }
                let nested =
                    tool.get("tools").and_then(Value::as_array).ok_or_else(|| anyhow!("Invalid ChatGPT tools"))?;
                for function in nested {
                    validate_tool(function, &mut names)?;
                    functions.push(function.clone());
                }
            } else {
                validate_tool(tool, &mut names)?;
                functions.push(tool.clone());
            }
            if functions.len() > 256 {
                bail!("Too many ChatGPT tools");
            }
        }
        object.insert(
            "tools".into(),
            if functions.is_empty() {
                json!([])
            } else {
                json!([{"type":"namespace","name":"i","description":"Tools supplied by i","tools":functions}])
            },
        );
    }
    if let Some(choice) = object.get_mut("tool_choice") {
        if let Some(value) = choice.as_str() {
            if !matches!(value, "auto" | "none" | "required") || (value == "required" && names.is_empty()) {
                bail!("Invalid ChatGPT tool choice");
            }
        } else {
            let selection = choice.as_object_mut().ok_or_else(|| anyhow!("Invalid ChatGPT tool choice"))?;
            if selection.get("type").and_then(Value::as_str) != Some("function")
                || !selection.get("name").and_then(Value::as_str).is_some_and(|name| names.contains(name))
                || selection.get("namespace").is_some_and(|n| n != "i")
            {
                bail!("Invalid ChatGPT tool choice");
            }
            selection.insert("namespace".into(), json!("i"));
        }
    }
    Ok(body)
}

fn validate_tool(tool: &Value, names: &mut HashSet<String>) -> Result<()> {
    if tool.get("type").and_then(Value::as_str) != Some("function") || tool.get("namespace").is_some_and(|n| n != "i") {
        bail!("Only local function tools are supported");
    }
    let name = tool.get("name").and_then(Value::as_str).ok_or_else(|| anyhow!("Invalid ChatGPT tool name"))?;
    if name.is_empty()
        || name.len() > 64
        || !name.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
        || !names.insert(name.to_owned())
    {
        bail!("Invalid or duplicate ChatGPT tool name");
    }
    if tool.get("parameters").is_some_and(|p| !p.is_object()) {
        bail!("Invalid ChatGPT tool parameters");
    }
    Ok(())
}

fn content_type(response: &reqwest::Response) -> &'static str {
    let Some(value) = response.headers().get(reqwest::header::CONTENT_TYPE) else {
        return "missing";
    };
    let value = value.to_str().unwrap_or("").split(';').next().unwrap_or("").trim();
    [
        "text/event-stream",
        "application/json",
        "text/html",
        "text/plain",
        "application/problem+json",
        "application/octet-stream",
    ]
    .into_iter()
    .find(|known| value.eq_ignore_ascii_case(known))
    .unwrap_or("other")
}
async fn unexpected_content_type(response: reqwest::Response) -> SafeDiagnostic {
    let mime = content_type(&response);
    let diagnostic = rejected_response(response).await;
    SafeDiagnostic(format!("ChatGPT response format failed (content_type={mime}); {diagnostic}; expected Responses SSE or completed JSON, not HTML, errors or partial tools"))
}
async fn send_response(auth: &Auth, body: Value) -> Result<reqwest::Response> {
    let body = prepare(body)
        .map_err(|_| SafeDiagnostic::stage("request preparation", "unsupported input/history or tool schema"))?;
    let token = auth.access_token().await.map_err(|error| {
        if error.downcast_ref::<SafeDiagnostic>().is_some() {
            return error;
        }
        SafeDiagnostic::stage(
            "authorization",
            "selected account/session or ChatGPT plan permission is unavailable; check login/scopes",
        )
        .into()
    })?;
    let response = tokio::time::timeout(
        DEADLINE,
        auth.inference_http.post("https://api.openai.com/v1/responses").bearer_auth(token).json(&body).send(),
    )
    .await
    .map_err(|_| SafeDiagnostic::stage("response connection", "request timed out; check network/proxy"))?
    .map_err(|error| {
        SafeDiagnostic::stage(
            "response connection",
            if error.is_timeout() {
                "network timeout; check proxy"
            } else if error.is_connect() {
                "connection/TLS/proxy failure"
            } else {
                "transport failure; check network/proxy"
            },
        )
    })?;
    if !response.status().is_success() {
        return Err(rejected_response(response).await.into());
    }
    Ok(response)
}
pub async fn response(auth: &Auth, body: Value) -> Result<reqwest::Response> {
    let response = send_response(auth, body).await?;
    if content_type(&response) != "text/event-stream" {
        return Err(unexpected_content_type(response).await.into());
    }
    Ok(response)
}

pub async fn complete(auth: &Auth, body: Value) -> Result<Value> {
    let body = prepare(body)
        .map_err(|_| SafeDiagnostic::stage("request preparation", "unsupported input/history or tool schema"))?;
    let mut response = send_response(auth, body.clone()).await?;
    let mime = content_type(&response);
    if !matches!(mime, "text/event-stream" | "application/json" | "missing") {
        return Err(unexpected_content_type(response).await.into());
    }
    tokio::time::timeout(DEADLINE, async {
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| SafeDiagnostic::stage("response stream", "stream read was interrupted; check network/proxy"))?
        {
            if bytes.len().saturating_add(chunk.len()) > MAX_STREAM {
                return Err(SafeDiagnostic::stage("response stream", "stream exceeds 48 MiB limit").into());
            }
            bytes.extend_from_slice(&chunk);
        }
        decode_complete(&bytes, &body, mime)
    })
    .await
    .map_err(|_| SafeDiagnostic::stage("response stream", "request timed out; no automatic replay"))?
}

fn decode_complete(bytes: &[u8], request: &Value, mime: &'static str) -> Result<Value> {
    let payload = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    let start = payload.trim_ascii_start();
    let format = if start.is_empty() {
        "empty"
    } else if start.starts_with(b"{") {
        "json"
    } else if [b"data:".as_slice(), b"event:", b":", b"id:", b"retry:"].iter().any(|prefix| start.starts_with(prefix)) {
        "sse"
    } else {
        "unrecognized"
    };
    let parsed = if bytes.len() > MAX_STREAM {
        Err(SafeDiagnostic::stage("response decoding", "response exceeds 48 MiB limit").into())
    } else if mime == "application/json" || mime == "missing" && format == "json" {
        parse_json(payload, request)
    } else if mime == "text/event-stream" || mime == "missing" && format == "sse" {
        parse_sse(payload, request)
    } else {
        Err(SafeDiagnostic::stage(
            "response decoding",
            if format == "empty" {
                "empty response body; check proxy/provider; no automatic replay"
            } else {
                "not a Responses SSE or JSON payload; pending tools were not executed"
            },
        )
        .into())
    };
    parsed.map_err(|error: anyhow::Error| {
        let safe = error
            .chain()
            .find_map(|error| error.downcast_ref::<SafeDiagnostic>())
            .map(ToString::to_string)
            .unwrap_or_else(|| "invalid or incomplete Responses payload; pending tools were not executed".into());
        SafeDiagnostic(format!(
            "ChatGPT response decoding failed (content_type={mime}; format={format}; bytes={}): {safe}",
            bytes.len()
        ))
        .into()
    })
}

fn parse_json(bytes: &[u8], request: &Value) -> Result<Value> {
    if bytes.len() > MAX_STREAM {
        return Err(SafeDiagnostic::stage("response JSON", "response exceeds 48 MiB").into());
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| SafeDiagnostic::stage("response JSON", "invalid JSON; pending tools were not executed"))?;
    if value.get("error").is_some_and(|error| !error.is_null()) || value.get("detail").is_some() {
        return Err(http_diagnostic(200, None, Some(&value)).into());
    }
    if value["object"] != "response"
        || value["status"] != "completed"
        || value["id"].as_str().is_none_or(str::is_empty)
        || value.get("incomplete_details").is_some_and(|details| !details.is_null())
    {
        return Err(SafeDiagnostic::stage(
            "response JSON",
            "not a completed Responses object; pending tools were not executed",
        )
        .into());
    }
    normalize(value, request).map_err(|_| {
        SafeDiagnostic::stage("response JSON", "invalid output or tool call; pending tools were not executed").into()
    })
}

fn parse_sse(bytes: &[u8], request: &Value) -> Result<Value> {
    if bytes.len() > MAX_STREAM {
        bail!(FAILURE);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| anyhow!(FAILURE))?.replace("\r\n", "\n");
    if !text.ends_with("\n\n") {
        bail!("ChatGPT response stream was interrupted");
    }
    let mut created: Option<String> = None;
    let mut completed = None;
    let mut done = false;
    for frame in text.split("\n\n") {
        let mut event_name = None;
        let mut data = Vec::new();
        for line in frame.lines() {
            if let Some(value) = line.strip_prefix("event:") {
                if event_name.replace(value.trim()).is_some() {
                    bail!(FAILURE);
                }
            } else if let Some(value) = line.strip_prefix("data:") {
                data.push(value.strip_prefix(' ').unwrap_or(value));
            } else if !line.is_empty()
                && !line.starts_with(':')
                && !line.starts_with("id:")
                && !line.starts_with("retry:")
            {
                bail!(FAILURE);
            }
        }
        if data.is_empty() {
            continue;
        }
        let data = data.join("\n");
        if data == "[DONE]" {
            if completed.is_none() || done {
                bail!(FAILURE);
            }
            done = true;
            continue;
        }
        if completed.is_some() || done {
            bail!(FAILURE);
        }
        let event: Value = serde_json::from_str(&data).map_err(|_| anyhow!(FAILURE))?;
        let kind = event.get("type").and_then(Value::as_str).ok_or_else(|| anyhow!(FAILURE))?;
        let failed_event = match kind {
            "error" => Some("error"),
            "response.error" => Some("response.error"),
            "response.failed" => Some("response.failed"),
            "response.incomplete" => Some("response.incomplete"),
            _ => None,
        };
        if let Some(kind) = failed_event {
            let error = event.pointer("/response/error").or_else(|| event.get("error")).unwrap_or(&event);
            let diagnostic = http_diagnostic(200, None, Some(&json!({"error":error})));
            let reason = event
                .pointer("/response/incomplete_details/reason")
                .and_then(Value::as_str)
                .filter(|reason| matches!(*reason, "max_output_tokens" | "content_filter"))
                .unwrap_or("unrecognized");
            return Err(SafeDiagnostic(format!("ChatGPT stream event={kind}; incomplete_reason={reason}; {diagnostic}; pending tools were not executed")).into());
        }
        if event.get("error").is_some_and(|error| !error.is_null())
            || event.pointer("/response/error").is_some_and(|error| !error.is_null())
            || event.pointer("/response/incomplete_details").is_some_and(|details| !details.is_null())
        {
            bail!(FAILURE);
        }
        if event_name.is_some_and(|name| name != kind) {
            bail!(FAILURE);
        }
        if matches!(kind, "error" | "response.error" | "response.failed" | "response.incomplete") {
            bail!(FAILURE);
        }
        if let Some(id) = event.get("response_id") {
            if id.as_str() != created.as_deref() || created.is_none() {
                bail!(FAILURE);
            }
        }
        if kind == "response.created" {
            let id = event
                .pointer("/response/id")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| anyhow!(FAILURE))?;
            if created.replace(id.to_owned()).is_some() {
                bail!(FAILURE);
            }
        } else {
            if created.is_none() {
                bail!(FAILURE);
            }
            if let Some(response) = event.get("response") {
                if response.get("id").and_then(Value::as_str) != created.as_deref() {
                    bail!(FAILURE);
                }
            }
        }
        if kind == "response.completed" {
            let response = event
                .get("response")
                .filter(|r| r.get("status").and_then(Value::as_str) == Some("completed"))
                .ok_or_else(|| anyhow!(FAILURE))?;
            completed = Some(response.clone());
        }
    }
    normalize(completed.ok_or_else(|| anyhow!("ChatGPT response stream ended without completion"))?, request)
}

fn normalize(response: Value, request: &Value) -> Result<Value> {
    let output = response.get("output").and_then(Value::as_array).ok_or_else(|| anyhow!(FAILURE))?;
    let allowed: HashSet<_> = request
        .get("tools")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|n| n.get("tools").and_then(Value::as_array))
        .flatten()
        .filter_map(|t| t.get("name").and_then(Value::as_str))
        .collect();
    let mut content = Vec::new();
    let mut calls = Vec::new();
    let mut ids = HashSet::new();
    let mut item_ids = HashSet::new();
    for item in output {
        if let Some(id) = item.get("id").and_then(Value::as_str) {
            if id.is_empty() || !item_ids.insert(id) {
                bail!(FAILURE);
            }
        }
        match item.get("type").and_then(Value::as_str) {
            Some("message") => {
                if item.get("status").is_some_and(|s| s != "completed")
                    || item.get("role").is_some_and(|r| r != "assistant")
                {
                    bail!(FAILURE);
                }
                for part in item.get("content").and_then(Value::as_array).ok_or_else(|| anyhow!(FAILURE))? {
                    match part.get("type").and_then(Value::as_str) {
                        Some("output_text") => {
                            content.push(part.get("text").and_then(Value::as_str).ok_or_else(|| anyhow!(FAILURE))?)
                        }
                        Some("refusal") => {
                            content.push(part.get("refusal").and_then(Value::as_str).ok_or_else(|| anyhow!(FAILURE))?)
                        }
                        _ => bail!(FAILURE),
                    }
                }
            }
            Some("function_call") => {
                let name = item.get("name").and_then(Value::as_str).ok_or_else(|| anyhow!(FAILURE))?;
                let id = item
                    .get("call_id")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| anyhow!(FAILURE))?;
                let arguments = item.get("arguments").and_then(Value::as_str).ok_or_else(|| anyhow!(FAILURE))?;
                if item.get("status").and_then(Value::as_str) != Some("completed")
                    || item.get("namespace").is_some_and(|n| n != "i")
                    || !allowed.contains(name)
                    || !ids.insert(id)
                    || !serde_json::from_str::<Value>(arguments).map_err(|_| anyhow!(FAILURE))?.is_object()
                {
                    bail!("ChatGPT returned an invalid local tool call");
                }
                calls.push(json!({"id":id,"type":"function","function":{"name":name,"arguments":arguments}}));
            }
            Some("reasoning") => {}
            _ => bail!("ChatGPT returned an unsupported output item"),
        }
    }
    let usage = response.get("usage").cloned().unwrap_or_else(|| json!({}));
    Ok(
        json!({"id":response.get("id"),"object":"chat.completion","model":request.get("model"),"choices":[{"index":0,"finish_reason":if calls.is_empty() {"stop"} else {"tool_calls"},"message":{"role":"assistant","content":content.join("\n"),"tool_calls":calls,"_responses_output":output}}],"usage":{"prompt_tokens":usage.get("input_tokens").and_then(Value::as_u64).unwrap_or(0),"completion_tokens":usage.get("output_tokens").and_then(Value::as_u64).unwrap_or(0),"total_tokens":usage.get("total_tokens").and_then(Value::as_u64).unwrap_or(0)}}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completed_json_responses_are_accepted_but_errors_and_incomplete_tools_are_not() {
        let request = prepare(json!({"model":"fixture","input":[]})).unwrap();
        let response = json!({"object":"response","id":"r1","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"OK"}]}]});
        let result = parse_json(&serde_json::to_vec(&response).unwrap(), &request).unwrap();
        assert_eq!(result["choices"][0]["message"]["content"], "OK");
        for bad in [
            json!({"choices":[{"message":{"content":"fake chat completion"}}]}),
            json!({"object":"response","id":"r1","status":"incomplete","output":[]}),
            json!({"object":"response","id":"r1","status":"completed","output":[{"type":"function_call","name":"unrequested","call_id":"c1","arguments":"{}"}]}),
            json!({"error":{"code":"model_not_found","message":"PRIVATE_PROMPT"}}),
        ] {
            let error = parse_json(&serde_json::to_vec(&bad).unwrap(), &request).unwrap_err();
            assert!(!worker_diagnostic(&error).contains("PRIVATE_PROMPT"));
        }
    }
    #[test]
    fn worker_reports_only_typed_safe_diagnostics_and_never_response_secrets() {
        let body = json!({"error":{"code":"subscription_sharing_unsupported_capability","param":"reasoning.effort","message":"PRIVATE_TOKEN_AND_PROMPT"}});
        let error = http_diagnostic(400, Some("req_fixture"), Some(&body));
        let displayed = worker_diagnostic(&anyhow::Error::new(error).context("untrusted PRIVATE_TOKEN_AND_PROMPT"));
        assert!(displayed.contains("HTTP 400") && displayed.contains("subscription_sharing_unsupported_capability"));
        assert!(displayed.contains("reasoning.effort") && displayed.contains("req_fixture"));
        assert!(!displayed.contains("PRIVATE_TOKEN_AND_PROMPT"));
        let error = http_diagnostic(403, Some("PRIVATE_TOKEN"), Some(&json!({"detail":"PRIVATE_TOKEN"})));
        let displayed = error.to_string();
        assert!(displayed.contains("HTTP 403") && displayed.contains("detail"));
        assert!(!displayed.contains("PRIVATE_TOKEN"));
        assert!(!worker_diagnostic(&anyhow!("PRIVATE_TOKEN")).contains("PRIVATE_TOKEN"));
        let body = json!({"error":{"code":"PRIVATE_TOKEN","param":"PRIVATE_TOKEN"}});
        assert!(!http_diagnostic(400, None, Some(&body)).to_string().contains("PRIVATE_TOKEN"));
    }
    #[test]
    fn failed_and_incomplete_stream_events_keep_only_safe_metadata() {
        let body = prepare(json!({"model":"fixture","input":[]})).unwrap();
        for (kind, error, reason) in [
            ("response.failed", json!({"code":"model_not_found","message":"PRIVATE_PROMPT"}), Value::Null),
            ("response.incomplete", Value::Null, json!({"reason":"max_output_tokens","message":"PRIVATE_PROMPT"})),
        ] {
            let bytes = format!(
                "data: {}\n\ndata: {}\n\n",
                json!({"type":"response.created","response":{"id":"r1"}}),
                json!({"type":kind,"response":{"id":"r1","error":error,"incomplete_details":reason}})
            );
            let error = parse_sse(bytes.as_bytes(), &body).unwrap_err();
            let text = worker_diagnostic(&error);
            assert!(text.contains(kind) && !text.contains("PRIVATE_PROMPT"), "{text}");
            assert!(text.contains("model_not_found") || text.contains("max_output_tokens"));
        }
    }
    use serde_json::json;

    #[test]
    fn multiple_output_parts_normalize_with_newlines() {
        let output = json!([{"type":"message","status":"completed","role":"assistant","content":[{"type":"output_text","text":"first"},{"type":"refusal","refusal":"second"}]},{"type":"message","status":"completed","role":"assistant","content":[{"type":"output_text","text":"third"}]}]);
        let response = normalize(json!({"id":"r","output":output}), &json!({"model":"test"})).unwrap();
        assert_eq!(response["choices"][0]["message"]["content"], "first\nsecond\nthird");
    }

    #[test]
    fn prepares_plan_request_and_preserves_encrypted_reasoning() {
        let prepared = prepare(json!({"model":"test", "store":true,"stream":false,"temperature":1,"max_output_tokens":10,"previous_response_id":"old", "input":[{"type":"message","role":"system","content":"rules"},{"type":"reasoning","encrypted_content":"opaque"},{"type":"function_call","name":"read","call_id":"call1","arguments":"{}"}],"tools":[{"type":"function","name":"read","parameters":{"type":"object"}}]})).unwrap();
        assert_eq!(prepared["store"], false);
        assert_eq!(prepared["stream"], true);
        assert!(prepared.get("temperature").is_none());
        assert!(prepared.get("max_output_tokens").is_none());
        assert!(prepared.get("previous_response_id").is_none());
        assert_eq!(prepared["input"][0]["role"], "developer");
        assert_eq!(prepared["input"][1]["encrypted_content"], "opaque");
        assert_eq!(prepared["input"][2]["namespace"], "i");
        assert_eq!(prepared["tools"][0]["type"], "namespace");
        assert_eq!(prepared["tools"][0]["tools"][0]["name"], "read");
    }

    #[test]
    fn rejects_untrusted_tools_and_bad_inputs() {
        for tools in [
            json!([{"type":"web_search"}]),
            json!([{"type":"mcp","server_url":"https://other.invalid"}]),
            json!([{"type":"namespace","name":"other","tools":[]}]),
            json!([{"type":"function","name":"read"},{"type":"function","name":"read"}]),
        ] {
            assert!(prepare(json!({"model":"test","input":[],"tools":tools})).is_err());
        }
        for value in [
            json!({"model":" ","input":[]}),
            json!({"model":"test","input":"text"}),
            json!({"model":"test","input":[],"tool_choice":{"type":"function","name":"missing"}}),
            json!({"model":"test","input":[{"type":"function_call","namespace":"other","name":"read"}]}),
        ] {
            assert!(prepare(value).is_err());
        }
    }

    fn stream(output: serde_json::Value) -> Vec<u8> {
        format!("event: response.created\ndata: {}\n\nevent: response.completed\ndata: {}\n\n", json!({"type":"response.created","response":{"id":"resp1","status":"in_progress"}}), json!({"type":"response.completed","response":{"id":"resp1","status":"completed","output":output,"usage":{"input_tokens":2,"output_tokens":3,"total_tokens":5}}})).into_bytes()
    }

    #[test]
    fn completed_sse_normalizes_text_and_function_calls() {
        let body = prepare(json!({"model":"test","input":[],"tools":[{"type":"function","name":"read"}]})).unwrap();
        let bytes = stream(
            json!([{"type":"message","status":"completed","content":[{"type":"output_text","text":"hello"}]},{"type":"function_call","status":"completed","namespace":"i","name":"read","id":"item1","call_id":"call1","arguments":"{\"path\":\"README.md\"}"}]),
        );
        let result = parse_sse(&bytes, &body).unwrap();
        assert_eq!(result["choices"][0]["message"]["content"], "hello");
        assert_eq!(result["choices"][0]["finish_reason"], "tool_calls");
        assert_eq!(result["choices"][0]["message"]["tool_calls"][0]["id"], "call1");
        assert_eq!(result["usage"]["prompt_tokens"], 2);
    }

    #[test]
    fn sse_rejects_failures_truncation_and_unsafe_tools() {
        let body = prepare(json!({"model":"test","input":[],"tools":[{"type":"function","name":"read"}]})).unwrap();
        for bytes in [
            b"data: {\"type\":\"response.failed\"}\n\n".to_vec(),
            b"data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp1\"}}\n\n".to_vec(),
            stream(
                json!([{"type":"function_call","status":"completed","name":"evil","call_id":"call1","arguments":"{}"}]),
            ),
            stream(
                json!([{"type":"function_call","status":"completed","name":"read","call_id":"call1","arguments":"[]"}]),
            ),
            stream(
                json!([{"type":"function_call","status":"in_progress","name":"read","call_id":"call1","arguments":"{}"}]),
            ),
        ] {
            assert!(parse_sse(&bytes, &body).is_err());
        }
        let good = stream(json!([]));
        let mut duplicate = good.clone();
        duplicate.extend_from_slice(&good);
        assert!(parse_sse(&duplicate, &body).is_err());
        let wrong_id = String::from_utf8(good.clone()).unwrap().replacen(
            "\"id\":\"resp1\",\"output\"",
            "\"id\":\"other\",\"output\"",
            1,
        );
        assert!(parse_sse(wrong_id.as_bytes(), &body).is_err());
        assert!(parse_sse(&good[..good.len() - 2], &body).is_err());
    }

    #[test]
    fn sse_preserves_reasoning_and_handles_crlf_multiline_frames() {
        let body = prepare(json!({"model":"test","input":[]})).unwrap();
        let bytes = stream(
            json!([{"type":"reasoning","id":"reason1","encrypted_content":"opaque"},{"type":"message","status":"completed","role":"assistant","content":[{"type":"output_text","text":"hello \u{1f30d}"}]}]),
        );
        let fixture = String::from_utf8(bytes).unwrap().replace("\n", "\r\n");
        let result = parse_sse(fixture.as_bytes(), &body).unwrap();
        assert_eq!(result["choices"][0]["message"]["_responses_output"][0]["encrypted_content"], "opaque");
        assert_eq!(result["choices"][0]["message"]["content"], "hello \u{1f30d}");
        let multiline = b": heartbeat\n\nevent: response.created\ndata: {\"type\":\"response.created\",\ndata: \"response\":{\"id\":\"r\"}}\n\nevent: response.completed\ndata: {\"type\":\"response.completed\",\ndata: \"response\":{\"id\":\"r\",\"status\":\"completed\",\"output\":[]}}\n\n";
        assert!(parse_sse(multiline, &body).is_ok());
    }

    #[test]
    fn sse_rejects_duplicate_call_ids_foreign_namespace_and_post_completion_errors() {
        let body = prepare(json!({"model":"test","input":[],"tools":[{"type":"function","name":"read"}]})).unwrap();
        let call = json!({"type":"function_call","status":"completed","name":"read","namespace":"i","call_id":"c","arguments":"{}"});
        assert!(parse_sse(&stream(json!([call.clone(), call.clone()])), &body).is_err());
        let mut foreign = call;
        foreign["namespace"] = json!("foreign");
        assert!(parse_sse(&stream(json!([foreign])), &body).is_err());
        let mut late_error = stream(json!([]));
        late_error.extend_from_slice(b"data: {\"type\":\"error\",\"message\":\"secret server detail\"}\n\n");
        let error = parse_sse(&late_error, &body).unwrap_err().to_string();
        assert!(!error.contains("secret server detail"));
        let missing_created = b"data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r\",\"status\":\"completed\",\"output\":[]}}\n\n";
        assert!(parse_sse(missing_created, &body).is_err());
        for kind in ["response.failed", "response.incomplete", "error"] {
            assert!(parse_sse(format!("data: {{\"type\":\"{kind}\"}}\n\n").as_bytes(), &body).is_err());
        }
    }

    #[test]
    fn prepare_is_idempotent_and_validates_selected_function() {
        let body = prepare(json!({"model":"test","input":[],"tools":[{"type":"function","name":"read","parameters":{"type":"object"}}],"tool_choice":{"type":"function","name":"read"}})).unwrap();
        assert_eq!(body["tool_choice"]["namespace"], "i");
        assert_eq!(prepare(body.clone()).unwrap(), body);
        assert!(prepare(json!({"model":"test","input":vec![json!({"role":"user","content":"x"});4097]})).is_err());
        assert!(prepare(json!({"model":"x".repeat(257),"input":[]})).is_err());
    }

    #[test]
    fn completed_status_cannot_hide_an_explicit_response_error() {
        let body = prepare(json!({"model":"test","input":[]})).unwrap();
        let bytes = String::from_utf8(stream(json!([])))
            .unwrap()
            .replace("\"status\":\"completed\"", "\"status\":\"completed\",\"error\":{\"message\":\"private\"}");
        assert!(parse_sse(bytes.as_bytes(), &body).is_err());
    }
}
