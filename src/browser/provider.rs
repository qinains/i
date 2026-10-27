use crate::{platform::Capabilities, workspace::tool_schemas};
use anyhow::{Result, ensure};
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
    pub reasoning_effort: String,
    pub use_max_tokens: bool,
    pub transport: String,
    pub stream: bool,
    pub relay_token: String,
    pub custom_headers: String,
}
impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            model: String::new(),
            api_key: String::new(),
            protocol: "openai".into(),
            max_tokens: 8192,
            reasoning_effort: "default".into(),
            use_max_tokens: true,
            transport: "direct".into(),
            stream: true,
            relay_token: String::new(),
            custom_headers: String::new(),
        }
    }
}
impl BrowserConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(matches!(self.protocol.as_str(), "openai" | "anthropic" | "responses"), "unsupported browser protocol");
        ensure!(matches!(self.transport.as_str(), "direct" | "relay" | "plugin"), "invalid transport");
        if self.transport == "plugin" {
            let url = url::Url::parse(&self.base_url)?;
            ensure!(
                matches!(self.protocol.as_str(), "responses" | "anthropic")
                    && self.stream
                    && self.custom_headers.trim().is_empty()
                    && self.api_key.len() >= 32
                    && url.scheme() == "http"
                    && url.host_str() == Some("127.0.0.1")
                    && url.port().is_some()
                    && url.path() == if self.protocol == "responses" { "/v1/responses" } else { "/v1/messages" },
                "invalid local connection plugin"
            );
        }
        ensure!(self.transport != "relay" || !self.relay_token.is_empty(), "relay token is required");
        ensure!(!self.model.trim().is_empty(), "model is required");
        ensure!(self.max_tokens > 0, "max_tokens must be positive");
        ensure!(
            matches!(self.reasoning_effort.as_str(), "default" | "none" | "low" | "medium" | "high" | "max" | "xhigh"),
            "unsupported reasoning effort"
        );
        if self.is_deepseek() {
            ensure!(
                !matches!(self.reasoning_effort.as_str(), "medium" | "xhigh"),
                "unsupported DeepSeek reasoning effort"
            );
            ensure!(self.max_tokens <= 393216, "DeepSeek max_tokens exceeds 393216");
        }
        self.endpoint()?;
        Ok(())
    }
    pub fn endpoint(&self) -> Result<String> {
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
        let path = url.path().trim_end_matches('/');
        ensure!(
            url.scheme() == "https" || matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")),
            "HTTPS is required except on loopback"
        );
        let path = match self.protocol.as_str() {
            "openai" if path.ends_with("/chat/completions") => path.to_owned(),
            "openai" => format!("{path}/chat/completions"),
            "anthropic" if path.ends_with("/v1/messages") => path.to_owned(),
            "anthropic" if path.ends_with("/v1") => format!("{path}/messages"),
            "anthropic" => format!("{path}/v1/messages"),
            "responses" if path.ends_with("/responses") => path.to_owned(),
            "responses" if path.is_empty() => "/v1/responses".into(),
            "responses" => format!("{path}/responses"),
            _ => anyhow::bail!("unsupported browser protocol"),
        };
        url.set_path(&path);
        Ok(url.into())
    }
    fn is_deepseek(&self) -> bool {
        self.protocol == "openai"
            && matches!(self.model.as_str(), "deepseek-flash" | "deepseek-v4-pro")
            && url::Url::parse(&self.base_url)
                .is_ok_and(|url| url.scheme() == "https" && url.host_str() == Some("api.deepseek.com"))
    }
    pub fn body(&self, history: &[Value]) -> Result<Value> {
        self.body_with_tools(history, &[])
    }
    pub fn analysis_body(&self, history: &[Value]) -> Result<Value> {
        let mut body = self.body_with_tools(history, &[])?;
        crate::analysis::disable_tools(&mut body)?;
        Ok(body)
    }
    pub fn analysis_response_text(&self, response: Value) -> Result<String> {
        crate::analysis::response_text(&self.normalize_response(response)?)
    }
    pub fn body_with_tools(&self, history: &[Value], extra: &[Value]) -> Result<Value> {
        self.validate()?;
        let mut definitions = tool_schemas(Capabilities::BROWSER).as_array().cloned().unwrap_or_default();
        ensure!(extra.len() <= 128, "too many plugin tools");
        for tool in extra {
            let name = tool["function"]["name"].as_str().ok_or_else(|| anyhow::anyhow!("invalid plugin schema"))?;
            ensure!(
                name.starts_with("p_")
                    && name.len() <= 64
                    && !definitions.iter().any(|d| d["function"]["name"] == name)
                    && tool["function"]["parameters"].is_object(),
                "invalid/duplicate plugin schema"
            );
            definitions.push(tool.clone());
        }
        if self.protocol == "anthropic" {
            return super::anthropic::body(self, history, &definitions);
        }
        if self.protocol == "responses" {
            return super::responses::body(self, history, &definitions);
        }
        let mut history = history.to_vec();
        for message in &mut history {
            if let Some(object) = message.as_object_mut() {
                object.retain(|key, _| !key.starts_with('_'));
            }
        }
        let mut body = json!({"model":self.model,"messages":history,"tools":definitions,"stream":self.stream});
        body[if self.use_max_tokens { "max_tokens" } else { "max_completion_tokens" }] = json!(self.max_tokens);
        if self.is_deepseek() {
            body.as_object_mut().unwrap().remove("max_completion_tokens");
            body["max_tokens"] = json!(self.max_tokens);
            if self.reasoning_effort != "default" {
                body["thinking"] = json!({"type":if self.reasoning_effort == "none" {"disabled"} else {"enabled"}});
                body["reasoning_effort"] = json!(self.reasoning_effort);
            }
        }
        Ok(body)
    }
    pub fn normalize_response(&self, response: Value) -> Result<Value> {
        self.normalize_response_with_tools(response, &[])
    }
    pub fn normalize_response_with_tools(&self, response: Value, extra: &[Value]) -> Result<Value> {
        match self.protocol.as_str() {
            "anthropic" => super::anthropic::normalize_response(response, extra),
            "responses" => super::responses::normalize_response(response, extra),
            _ => Ok(response),
        }
    }
}
