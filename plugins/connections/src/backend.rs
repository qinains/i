use crate::claude::{self, ClaudeAuth};
use anyhow::{anyhow, bail, Result};
use i_chatgpt::{inference, Auth};
use serde_json::{json, Value};
use std::collections::HashSet;

#[derive(Clone)]
pub enum Backend {
    Chatgpt(Auth),
    Claude(ClaudeAuth),
}

impl Backend {
    pub fn at(claude: bool, root: std::path::PathBuf) -> Result<Self> {
        if claude {
            Ok(Self::Claude(ClaudeAuth::at(root)?))
        } else {
            Ok(Self::Chatgpt(Auth::at(root)?))
        }
    }
    pub fn chatgpt() -> Result<Self> {
        Ok(Self::Chatgpt(Auth::new()?))
    }
    pub fn claude() -> Result<Self> {
        Ok(Self::Claude(ClaudeAuth::new()?))
    }
    pub fn manifest(&self) -> Value {
        match self {
            Self::Chatgpt(_) => {
                json!({"format_version":1,"kind":"connection","id":"openai.chatgpt","name":"ChatGPT","protocol":"responses"})
            }
            Self::Claude(_) => {
                json!({"format_version":1,"kind":"connection","id":"anthropic.claude","name":"Claude Console (API billing, not Pro/Max)","protocol":"anthropic"})
            }
        }
    }
    pub async fn status(&self) -> Result<Value> {
        match self {
            Self::Chatgpt(a) => Ok(serde_json::to_value(a.status().await?)?),
            Self::Claude(a) => a.status().await,
        }
    }
    pub async fn models(&self) -> Result<Value> {
        match self {
            Self::Chatgpt(a) => a.models().await,
            Self::Claude(a) => a.models().await,
        }
    }
    pub async fn login(&self, account: Option<&str>) -> Result<Value> {
        match self {
            Self::Chatgpt(a) => Ok(serde_json::to_value(a.login(account).await?)?),
            Self::Claude(a) => a.login(account).await,
        }
    }
    pub async fn select(&self, id: &str) -> Result<Value> {
        match self {
            Self::Chatgpt(a) => {
                a.select(id).await?;
                Ok(json!({"active":id}))
            }
            Self::Claude(a) => a.select(id).await,
        }
    }
    pub async fn logout(&self, account: Option<&str>) -> Result<Value> {
        match self {
            Self::Chatgpt(a) => Ok(json!({"signed_out":true,"remote_revocation_confirmed":a.logout(account).await?})),
            Self::Claude(a) => a.logout(account).await,
        }
    }
    pub async fn response(&self, body: Value) -> Result<reqwest::Response> {
        match self {
            Self::Chatgpt(a) => inference::response(a, body).await,
            Self::Claude(a) => a.response(body).await,
        }
    }
    pub async fn complete(&self, data: Value) -> Result<Value> {
        match self {
            Self::Chatgpt(a) => {
                inference::complete(
                    a,
                    responses_body(data).map_err(|_| {
                        inference::SafeDiagnostic::stage(
                            "request preparation",
                            "unsupported input/history or tool schema",
                        )
                    })?,
                )
                .await
            }
            Self::Claude(a) => {
                let body = claude::messages_body(data)?;
                a.complete(body).await
            }
        }
    }
}

pub fn field<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
    value.get(name).and_then(Value::as_str).ok_or_else(|| anyhow!("Invalid connection request"))
}

pub fn definitions(data: &Value) -> Result<Vec<Value>> {
    let mut result = Vec::new();
    let mut names = HashSet::new();
    for key in ["tools", "plugin_tools"] {
        if let Some(raw) = data.get(key) {
            for tool in raw.as_array().ok_or_else(|| anyhow!("Invalid connection tools"))? {
                let function = tool.get("function").unwrap_or(tool);
                let name = field(function, "name")?;
                if tool.get("type").is_some_and(|t| t != "function")
                    || name.is_empty()
                    || !names.insert(name.to_owned())
                {
                    bail!("Invalid connection tools");
                }
                result.push(json!({"type":"function","name":name,"description":function.get("description").cloned().unwrap_or(json!("")),"parameters":function.get("parameters").cloned().unwrap_or(json!({"type":"object","properties":{}})),"strict":false}));
            }
        }
    }
    if result.len() > 256 {
        bail!("Too many connection tools");
    }
    Ok(result)
}

pub fn calls(message: &Value) -> Result<Vec<Value>> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    if let Some(raw) = message.get("tool_calls").filter(|v| !v.is_null()) {
        for call in raw.as_array().ok_or_else(|| anyhow!("Invalid tool history"))? {
            let id = field(call, "id")?;
            let function = call.get("function").ok_or_else(|| anyhow!("Invalid tool history"))?;
            let name = field(function, "name")?;
            let arguments = field(function, "arguments")?;
            if id.is_empty()
                || name.is_empty()
                || !seen.insert(id)
                || !serde_json::from_str::<Value>(arguments).map_err(|_| anyhow!("Invalid tool history"))?.is_object()
            {
                bail!("Invalid tool history");
            }
            result.push(json!({"id":id,"type":"function","function":{"name":name,"arguments":arguments}}));
        }
    }
    Ok(result)
}

fn parts(content: &Value, assistant: bool) -> Result<Vec<Value>> {
    if content.is_null() {
        return Ok(vec![]);
    }
    if let Some(text) = content.as_str() {
        return Ok(vec![json!({"type":"input_text","text":text})]);
    }
    let mut output = Vec::new();
    for part in content.as_array().ok_or_else(|| anyhow!("Invalid message content"))? {
        match field(part, "type")? {
            "text" => output.push(json!({"type":"input_text","text":field(part,"text")?})),
            "image_url" if !assistant => {
                output.push(json!({"type":"input_image","image_url":field(&part["image_url"],"url")?}))
            }
            _ => bail!("Unsupported ChatGPT message content"),
        }
    }
    Ok(output)
}

pub fn responses_body(data: Value) -> Result<Value> {
    let mut input = Vec::new();
    for message in
        data.get("history").and_then(Value::as_array).ok_or_else(|| anyhow!("Missing conversation history"))?
    {
        match field(message,"role")? {
            "system" | "developer" | "user" => input.push(json!({"role":if message["role"] == "system" {"developer"} else {field(message,"role")?},"content":parts(&message["content"],false)?})),
            "tool" => input.push(json!({"type":"function_call_output","call_id":field(message,"tool_call_id")?,"output":parts(&message["content"],false)?})),
            "assistant" => {
                if message.get("_anthropic_content").is_some() { bail!("Cannot replay Claude reasoning through ChatGPT"); }
                let canonical = calls(message)?;
                if let Some(saved) = message.get("_responses_output") {
                    let saved = saved.as_array().ok_or_else(|| anyhow!("Invalid saved response history"))?;
                    let mut replay = Vec::new(); let mut text = Vec::new(); let mut ids = HashSet::new();
                    for item in saved {
                        if let Some(id) = item.get("id").and_then(Value::as_str) { if id.is_empty() || !ids.insert(id) { bail!("Invalid saved response history"); } }
                        match field(item,"type")? {
                            "message" => {
                                if item["status"] != "completed" || item["role"] != "assistant" { bail!("Invalid saved response history"); }
                                for part in item["content"].as_array().ok_or_else(|| anyhow!("Invalid saved response history"))? {
                                    text.push(match field(part,"type")? {"output_text" => field(part,"text")?, "refusal" => field(part,"refusal")?, _ => bail!("Invalid saved response history")});
                                }
                            },
                            "function_call" => {
                                if item["status"] != "completed" || item.get("namespace").is_some_and(|n| n != "i") { bail!("Invalid saved response history"); }
                                replay.push(json!({"id":field(item,"call_id")?,"type":"function","function":{"name":field(item,"name")?,"arguments":field(item,"arguments")?}}));
                            },
                            "reasoning" => {
                                if item.get("encrypted_content").is_some_and(|v| !v.is_null() && !v.is_string()) || item.get("status").is_some_and(|s| !s.is_null() && s != "completed") { bail!("Invalid saved reasoning"); }
                            },
                            _ => bail!("Unsupported saved response history"),
                        }
                    }
                    if replay != canonical || message["content"].as_str().unwrap_or("") != text.join("\n") { bail!("Saved response history does not match conversation"); }
                    input.extend(saved.iter().cloned());
                } else {
                    if !message["content"].is_null() { input.push(json!({"role":"assistant","content":parts(&message["content"],true)?})); }
                    for call in canonical { input.push(json!({"type":"function_call","namespace":"i","call_id":call["id"],"name":call["function"]["name"],"arguments":call["function"]["arguments"]})); }
                }
            },
            _ => bail!("Unsupported conversation role"),
        }
    }
    let mut body = json!({"model":field(&data,"model")?,"input":input,"tools":definitions(&data)?,"include":["reasoning.encrypted_content"],"store":false,"stream":true});
    if let Some(extra) = data.get("extra_body").filter(|v| !v.is_null()) {
        for (key, value) in extra.as_object().ok_or_else(|| anyhow!("Invalid extra body"))? {
            if matches!(key.as_str(), "reasoning" | "text" | "parallel_tool_calls" | "tool_choice") {
                body[key] = value.clone();
            }
        }
    }
    if data["compact"] == true {
        let object = body.as_object_mut().ok_or_else(|| anyhow!("Invalid compact request"))?;
        object.remove("tools");
        object.remove("tool_choice");
        object.remove("parallel_tool_calls");
    }
    inference::prepare(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifests_match_strict_connection_schema() {
        let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
        for claude in [false, true] {
            let manifest =
                Backend::at(claude, dir.path().join(if claude { "claude" } else { "chatgpt" })).unwrap().manifest();
            let mut keys = manifest.as_object().unwrap().keys().map(String::as_str).collect::<Vec<_>>();
            keys.sort_unstable();
            assert_eq!(keys, vec!["format_version", "id", "kind", "name", "protocol"]);
            assert_eq!(manifest["format_version"], 1);
        }
    }

    #[test]
    fn compact_requests_do_not_advertise_tools() {
        let data = json!({"model":"test","history":[{"role":"user","content":"summarize"}],"compact":true,"tools":[{"type":"function","function":{"name":"read","parameters":{"type":"object"}}}],"plugin_tools":[{"name":"custom","parameters":{"type":"object"}}],"extra_body":{"tool_choice":{"type":"function","name":"read"}}});
        for body in [responses_body(data.clone()).unwrap(), claude::messages_body(data).unwrap()] {
            assert!(body.get("tools").is_none());
            assert!(body.get("tool_choice").is_none());
        }
    }

    #[test]
    fn responses_history_keeps_namespace_tools_and_images() {
        let body = responses_body(json!({"model":"test","history":[{"role":"system","content":"rules"},{"role":"user","content":[{"type":"image_url","image_url":{"url":"data:image/png;base64,aA=="}}]},{"role":"assistant","content":"","tool_calls":[{"id":"c","type":"function","function":{"name":"read","arguments":"{}"}}]},{"role":"tool","tool_call_id":"c","content":"result"}],"tools":[{"type":"function","function":{"name":"read","parameters":{"type":"object"}}}],"plugin_tools":[]})).unwrap();
        assert_eq!(body["input"][0]["role"], "developer");
        assert_eq!(body["input"][1]["content"][0]["type"], "input_image");
        assert!(body["input"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["type"] == "function_call" && i["namespace"] == "i"));
        assert_eq!(body["tools"][0]["tools"][0]["name"], "read");
    }

    #[test]
    fn mismatched_saved_tool_history_is_rejected() {
        assert!(responses_body(json!({"model":"test","history":[{"role":"assistant","content":"","tool_calls":[],"_responses_output":[{"type":"function_call","name":"evil","call_id":"c","arguments":"{}","status":"completed"}]}],"tools":[],"plugin_tools":[]})).is_err());
    }

    #[test]
    fn multiple_output_parts_replay_with_newline_normalized_content() {
        let saved = json!([{"id":"m1","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"first"},{"type":"refusal","refusal":"second"}]},{"id":"m2","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"third"}]}]);
        let body=responses_body(json!({"model":"test","history":[{"role":"assistant","content":"first\nsecond\nthird","tool_calls":[],"_responses_output":saved}]})).unwrap();
        assert_eq!(body["input"], saved);
    }
}
