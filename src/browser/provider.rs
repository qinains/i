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
    pub max_steps: usize,
    pub use_max_tokens: bool,
}
impl Default for BrowserConfig {
    fn default() -> Self {
        Self { base_url: String::new(), model: String::new(), api_key: String::new(), protocol: "openai".into(),
            max_tokens: 8192, max_steps: 32, use_max_tokens: true }
    }
}
impl BrowserConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.protocol == "openai", "browser supports only OpenAI-compatible Chat Completions");
        ensure!(!self.model.trim().is_empty(), "model is required");
        ensure!(self.max_tokens > 0 && (1..=32).contains(&self.max_steps), "invalid max_tokens or max_steps (1..32)");
        self.endpoint()?; Ok(())
    }
    pub fn endpoint(&self) -> Result<String> {
        let mut url = url::Url::parse(&self.base_url)?;
        ensure!(matches!(url.scheme(), "http" | "https") && url.host_str().is_some()
            && url.username().is_empty() && url.password().is_none() && url.query().is_none() && url.fragment().is_none(),
            "base_url must be an HTTP(S) URL without credentials, query or fragment");
        let path = url.path().trim_end_matches('/');
        if !path.ends_with("/chat/completions") { url.set_path(&format!("{path}/chat/completions")); }
        else { url.set_path(&path.to_owned()); }
        Ok(url.into())
    }
    pub fn body(&self, history: &[Value]) -> Result<Value> {
        self.validate()?;
        let mut body = json!({"model":self.model,"messages":history,"tools":tool_schemas(Capabilities::BROWSER),"stream":false});
        body[if self.use_max_tokens { "max_tokens" } else { "max_completion_tokens" }] = json!(self.max_tokens);
        Ok(body)
    }
}
