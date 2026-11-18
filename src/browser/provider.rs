use crate::{agent::history::tool_calls, platform::Capabilities, workspace::tool_schemas};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BrowserConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    pub protocol: String,
    pub max_tokens: usize,
    pub max_steps: usize,
    pub use_max_tokens: bool,
    pub transport: String,
    pub stream: bool,
    pub relay_token: String,
    pub reasoning_effort: String,
    pub custom_headers: String,
    pub language: String,
}
impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            model: String::new(),
            api_key: String::new(),
            protocol: "openai".into(),
            max_tokens: 8192,
            max_steps: 32,
            use_max_tokens: true,
            transport: "direct".into(),
            stream: true,
            relay_token: String::new(),
            reasoning_effort: "default".into(),
            custom_headers: String::new(),
            language: "en".into(),
        }
    }
}
impl BrowserConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(matches!(self.protocol.as_str(), "openai" | "anthropic"), "unsupported browser protocol");
        ensure!(
            matches!(self.reasoning_effort.as_str(), "default" | "none" | "low" | "high" | "max"),
            "invalid reasoning_effort"
        );
        ensure!(matches!(self.transport.as_str(), "direct" | "relay"), "invalid transport");
        ensure!(
            matches!(
                self.language.as_str(),
                "en" | "zh-CN"
                    | "zh-TW"
                    | "ja"
                    | "ko"
                    | "fr"
                    | "de"
                    | "es"
                    | "pt"
                    | "ru"
                    | "ar"
                    | "it"
                    | "nl"
                    | "pl"
                    | "tr"
                    | "vi"
                    | "th"
            ),
            "invalid language"
        );
        ensure!(self.transport != "relay" || !self.relay_token.is_empty(), "relay token is required");
        ensure!(!self.model.trim().is_empty(), "model is required");
        ensure!(self.max_tokens > 0 && (1..=32).contains(&self.max_steps), "invalid max_tokens or max_steps (1..32)");
        ensure!(self.max_tokens as u128 <= 9_007_199_254_740_991, "max_tokens must be a safe integer");
        ensure!(
            !self.is_deepseek()
                || !matches!(self.model.as_str(), "deepseek-flash" | "deepseek-v4-pro")
                || self.max_tokens <= 393216,
            "DeepSeek max_tokens exceeds model maximum"
        );
        self.validate_custom_headers()?;
        self.endpoint()?;
        Ok(())
    }
    pub fn endpoint(&self) -> Result<String> {
        ensure!(matches!(self.protocol.as_str(), "openai" | "anthropic"), "unsupported browser protocol");
        let mut url = url::Url::parse(&self.base_url)?;
        ensure!(
            matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none(),
            "base_url must be an HTTP(S) URL without credentials, query or fragment"
        );
        let path = url.path().trim_end_matches('/').to_owned();
        ensure!(
            url.scheme() == "https" || matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")),
            "HTTPS is required except on loopback"
        );
        if self.protocol == "anthropic" && !path.ends_with("/v1/messages") {
            url.set_path(&format!("{path}{}", if path.ends_with("/v1") { "/messages" } else { "/v1/messages" }));
        } else if self.protocol == "openai" && !path.ends_with("/chat/completions") {
            url.set_path(&format!("{path}/chat/completions"));
        } else {
            url.set_path(&path);
        }
        Ok(url.into())
    }
    pub fn body(&self, history: &[Value]) -> Result<Value> {
        self.body_with_tools(history, &[])
    }

    pub fn body_with_tools(&self, history: &[Value], extra: &[Value]) -> Result<Value> {
        self.validate()?;
        let mut definitions =
            tool_schemas(Capabilities::BROWSER).as_array().context("invalid browser tool schemas")?.clone();
        definitions.extend_from_slice(extra);
        if self.protocol == "anthropic" {
            return super::anthropic::body(self, history, &definitions);
        }
        let history: Vec<_> = history.iter().cloned().map(strip_private_metadata).collect();
        let mut body = json!({"model":self.model,"messages":history,"tools":definitions,"stream":self.stream});
        body[if self.use_max_tokens || self.is_deepseek() { "max_tokens" } else { "max_completion_tokens" }] =
            json!(self.max_tokens);
        if self.is_deepseek() && self.reasoning_effort != "default" {
            body["reasoning_effort"] = json!(self.reasoning_effort);
            body["thinking"] = json!({"type":if self.reasoning_effort == "none" { "disabled" } else { "enabled" }});
        }
        Ok(body)
    }

    pub fn normalize_response(&self, response: Value) -> Result<Value> {
        self.normalize_response_with_tools(response, &[])
    }

    pub fn normalize_response_with_tools(&self, mut response: Value, extra: &[Value]) -> Result<Value> {
        if self.protocol == "anthropic" {
            return super::anthropic::normalize_response(response, extra);
        }
        ensure!(self.protocol == "openai", "unsupported browser protocol");
        let choice = response["choices"]
            .as_array_mut()
            .and_then(|choices| choices.first_mut())
            .context("missing completion choice")?;
        let reason = choice["finish_reason"].as_str().context("missing finish_reason")?;
        ensure!(matches!(reason, "stop" | "tool_calls"), "incomplete OpenAI response: unsupported finish_reason");
        let message = &choice["message"];
        ensure!(message["role"] == "assistant", "invalid assistant response");
        let calls = tool_calls(message)?;
        ensure!((reason == "tool_calls") != calls.is_empty(), "incomplete OpenAI tool batch");
        let definitions = tool_schemas(Capabilities::BROWSER);
        let definitions = definitions.as_array().context("invalid browser tool schemas")?;
        for call in &calls {
            let name = call["function"]["name"].as_str().context("missing tool name")?;
            ensure!(
                definitions.iter().chain(extra).any(|tool| tool["function"]["name"] == name),
                "unsupported tool in browser: {name}"
            );
        }
        choice["message"] = strip_private_metadata(message.clone());
        Ok(response)
    }

    fn is_deepseek(&self) -> bool {
        self.protocol == "openai"
            && url::Url::parse(&self.base_url)
                .is_ok_and(|url| url.scheme() == "https" && url.host_str() == Some("api.deepseek.com"))
    }

    fn validate_custom_headers(&self) -> Result<()> {
        let raw = self.custom_headers.trim();
        if raw.is_empty() {
            return Ok(());
        }
        let value: Value = serde_json::from_str(raw).context("invalid custom headers JSON")?;
        for (name, value) in value.as_object().context("custom headers must be a JSON object")? {
            let key = name.to_ascii_lowercase();
            let text = value.as_str().context("custom header values must be strings")?;
            ensure!(
                !key.is_empty()
                    && key.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
                    && !text.contains(['\r', '\n']),
                "invalid custom header"
            );
            ensure!(
                !matches!(
                    key.as_str(),
                    "authorization"
                        | "proxy-authorization"
                        | "host"
                        | "cookie"
                        | "origin"
                        | "content-type"
                        | "accept"
                        | "content-length"
                        | "connection"
                        | "x-api-key"
                        | "anthropic-version"
                        | "anthropic-dangerous-direct-browser-access"
                ) && !["x-harness-", "sec-", "proxy-"].iter().any(|prefix| key.starts_with(prefix)),
                "reserved custom header"
            );
        }
        Ok(())
    }
}

fn strip_private_metadata(mut message: Value) -> Value {
    if let Some(fields) = message.as_object_mut() {
        fields.retain(|key, _| !key.starts_with('_'));
    }
    message
}
