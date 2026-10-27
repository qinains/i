use crate::backend::{calls, definitions, field};
use anyhow::{anyhow, bail, Result};
use base64::Engine;
use fs2::FileExt;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File, OpenOptions},
    path::PathBuf,
    time::Duration,
};
use tokio::io::AsyncReadExt;

const FAILURE: &str = "Claude Console connection could not be completed";
const ACCOUNT: &str = "claude-console";
const LIMIT: usize = 48 * 1024 * 1024;

#[derive(Clone)]
pub struct ClaudeAuth {
    root: PathBuf,
}

fn safe_path(path: &std::path::Path) -> Result<()> {
    for p in path.ancestors().filter(|p| !p.as_os_str().is_empty()) {
        match fs::symlink_metadata(p) {
            Ok(meta) if meta.file_type().is_symlink() => bail!("Unsafe Claude plugin storage"),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => bail!("Cannot inspect Claude plugin storage"),
        }
    }
    Ok(())
}

impl ClaudeAuth {
    pub fn new() -> Result<Self> {
        Self::at(dirs_next::config_dir().ok_or_else(|| anyhow!(FAILURE))?.join("i").join("claude"))
    }
    pub fn at(root: PathBuf) -> Result<Self> {
        safe_path(&root)?;
        safe_path(&root.join("ant"))?;
        safe_path(&root.join("home"))?;
        fs::create_dir_all(root.join("ant")).map_err(|_| anyhow!(FAILURE))?;
        fs::create_dir_all(root.join("home")).map_err(|_| anyhow!(FAILURE))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for path in [&root, &root.join("ant"), &root.join("home")] {
                fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|_| anyhow!(FAILURE))?;
            }
        }
        Ok(Self { root })
    }
    fn account(id: Option<&str>) -> Result<()> {
        if id.is_some_and(|id| id != ACCOUNT) {
            bail!("Unknown Claude Console account; this plugin supports one workspace profile");
        }
        Ok(())
    }
    async fn lock(&self) -> Result<File> {
        let path = self.root.join("ant.lock");
        safe_path(&path)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
            #[cfg(target_os = "linux")]
            options.custom_flags(0x20000);
            #[cfg(target_os = "macos")]
            options.custom_flags(0x100);
        }
        let lock = options.open(path).map_err(|_| anyhow!(FAILURE))?;
        let start = tokio::time::Instant::now();
        loop {
            match lock.try_lock_exclusive() {
                Ok(()) => return Ok(lock),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock && start.elapsed() < Duration::from_secs(660) => {
                    tokio::time::sleep(Duration::from_millis(50)).await
                }
                Err(_) => bail!("Claude Console authentication is busy"),
            }
        }
    }
    fn command(&self, args: &[&str]) -> tokio::process::Command {
        let mut command = tokio::process::Command::new("ant");
        command.env_clear();
        for key in [
            "PATH",
            "SystemRoot",
            "WINDIR",
            "TMPDIR",
            "TEMP",
            "TMP",
            "DISPLAY",
            "WAYLAND_DISPLAY",
            "XDG_RUNTIME_DIR",
            "DBUS_SESSION_BUS_ADDRESS",
        ] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        command
            .env("HOME", self.root.join("home"))
            .env("USERPROFILE", self.root.join("home"))
            .env("ANTHROPIC_CONFIG_DIR", self.root.join("ant"))
            .env("ANTHROPIC_BASE_URL", "https://api.anthropic.com");
        command
            .args(args)
            .args(["--profile", "i"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        command
    }
    async fn run(&self, args: &[&str], seconds: u64) -> Result<String> {
        safe_path(&self.root.join("ant"))?;
        let mut command = self.command(args);
        let mut child = command.spawn().map_err(|_| anyhow!("Install the official ant CLI first: https://platform.claude.com/docs/en/cli-sdks-libraries/cli/quickstart"))?;
        tokio::time::timeout(Duration::from_secs(seconds), async {
            let mut stdout = child.stdout.take().ok_or_else(|| anyhow!(FAILURE))?.take(65_537);
            let mut bytes = Vec::new();
            stdout.read_to_end(&mut bytes).await.map_err(|_| anyhow!(FAILURE))?;
            if bytes.len() > 65_536 {
                let _ = child.kill().await;
                bail!(FAILURE);
            }
            if !child.wait().await.map_err(|_| anyhow!(FAILURE))?.success() {
                bail!(FAILURE);
            }
            String::from_utf8(bytes).map_err(|_| anyhow!(FAILURE))
        })
        .await
        .map_err(|_| anyhow!("Claude Console command timed out"))?
    }
    async fn token_locked(&self) -> Result<String> {
        if self.root.join("signed-out").try_exists().map_err(|_| anyhow!(FAILURE))? {
            bail!("Sign in to Claude Console first");
        }
        let token = self.run(&["auth", "print-credentials", "--access-token"], 60).await?;
        let token = token.trim();
        if token.is_empty() || !token.bytes().all(|b| b.is_ascii_graphic()) {
            bail!(FAILURE);
        }
        Ok(token.to_owned())
    }
    async fn token(&self) -> Result<String> {
        let _lock = self.lock().await?;
        self.token_locked().await
    }
    pub async fn login(&self, account: Option<&str>) -> Result<Value> {
        Self::account(account)?;
        let _lock = self.lock().await?;
        self.run(&["auth", "login"], 600).await?;
        let marker = self.root.join("signed-out");
        safe_path(&marker)?;
        if marker.exists() {
            fs::remove_file(marker).map_err(|_| anyhow!(FAILURE))?;
        }
        self.token_locked().await?;
        Ok(
            json!({"id":ACCOUNT,"label":"Claude Console API","email":null,"active":true,"signed_in":true,"billing":"Console API billing; Claude Pro/Max credits are not supported"}),
        )
    }
    pub async fn status(&self) -> Result<Value> {
        let signed_in = self.token().await.is_ok();
        Ok(
            json!({"active":if signed_in {json!(ACCOUNT)} else {Value::Null},"accounts":[{"id":ACCOUNT,"label":"Claude Console API","email":null,"active":signed_in,"signed_in":signed_in}],"billing":"Console API billing; Claude Pro/Max credits are not supported"}),
        )
    }
    pub async fn select(&self, id: &str) -> Result<Value> {
        Self::account(Some(id))?;
        self.token().await?;
        Ok(json!({"active":ACCOUNT}))
    }
    pub async fn logout(&self, account: Option<&str>) -> Result<Value> {
        self.logout_using(account, Duration::from_secs(20), || async {
            self.run(&["auth", "logout"], 20).await.is_ok()
        })
        .await
    }
    async fn logout_using<F, Fut>(&self, account: Option<&str>, remote_timeout: Duration, logout: F) -> Result<Value>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        Self::account(account)?;
        let _lock = self.lock().await?;
        let marker = self.root.join("signed-out");
        safe_path(&marker)?;
        let marker_file = tempfile::NamedTempFile::new_in(&self.root).map_err(|_| anyhow!(FAILURE))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            marker_file.as_file().set_permissions(fs::Permissions::from_mode(0o600)).map_err(|_| anyhow!(FAILURE))?;
        }
        marker_file.as_file().sync_all().map_err(|_| anyhow!(FAILURE))?;
        marker_file.persist(marker).map_err(|_| anyhow!(FAILURE))?;
        #[cfg(unix)]
        File::open(&self.root).and_then(|f| f.sync_all()).map_err(|_| anyhow!(FAILURE))?;
        let credentials = self.root.join("ant").join("credentials").join("i.json");
        safe_path(&credentials)?;
        if credentials.exists() {
            fs::remove_file(&credentials).map_err(|_| anyhow!(FAILURE))?;
            #[cfg(unix)]
            File::open(credentials.parent().ok_or_else(|| anyhow!(FAILURE))?)
                .and_then(|f| f.sync_all())
                .map_err(|_| anyhow!(FAILURE))?;
        }
        let confirmed = tokio::time::timeout(remote_timeout, logout()).await.unwrap_or(false);
        Ok(json!({"signed_out":true,"remote_revocation_confirmed":false,"official_cli_logout_completed":confirmed}))
    }
    fn client(&self) -> Result<reqwest::Client> {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(15))
            .build()
            .map_err(|_| anyhow!(FAILURE))
    }
    pub async fn models(&self) -> Result<Value> {
        let token = self.token().await?;
        let response = self
            .client()?
            .get("https://api.anthropic.com/v1/models")
            .bearer_auth(token)
            .header("anthropic-version", "2023-06-01")
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .map_err(|_| anyhow!(FAILURE))?;
        if !response.status().is_success() {
            bail!("Claude Console model catalog unavailable");
        }
        let body: Value = response.json().await.map_err(|_| anyhow!(FAILURE))?;
        let mut models = Vec::new();
        for model in body["data"].as_array().ok_or_else(|| anyhow!(FAILURE))? {
            models.push(json!({"slug":field(model,"id")?,"display_name":model.get("display_name").cloned().unwrap_or_else(|| model["id"].clone()),"visibility":"list"}));
        }
        Ok(json!({"models":models}))
    }
    pub async fn response(&self, mut body: Value) -> Result<reqwest::Response> {
        validate_body(&body)?;
        body["stream"] = json!(true);
        let token = self.token().await?;
        let response = tokio::time::timeout(
            Duration::from_secs(1800),
            self.client()?
                .post("https://api.anthropic.com/v1/messages")
                .bearer_auth(token)
                .header("anthropic-version", "2023-06-01")
                .json(&body)
                .send(),
        )
        .await
        .map_err(|_| anyhow!(FAILURE))?
        .map_err(|_| anyhow!(FAILURE))?;
        if !response.status().is_success() {
            bail!("Claude Console response request was not accepted");
        }
        if response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(';').next().unwrap_or("").trim())
            != Some("text/event-stream")
        {
            bail!(FAILURE);
        }
        Ok(response)
    }
    pub async fn complete(&self, body: Value) -> Result<Value> {
        let mut response = self.response(body.clone()).await?;
        tokio::time::timeout(Duration::from_secs(1800), async {
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|_| anyhow!(FAILURE))? {
                if bytes.len().saturating_add(chunk.len()) > LIMIT {
                    bail!("Claude response is too large");
                }
                bytes.extend_from_slice(&chunk);
            }
            parse_sse(&bytes, &body)
        })
        .await
        .map_err(|_| anyhow!("Claude response timed out"))?
    }
}

fn parts(content: &Value) -> Result<Vec<Value>> {
    if content.is_null() {
        return Ok(vec![]);
    }
    if let Some(text) = content.as_str() {
        return Ok(if text.is_empty() { vec![] } else { vec![json!({"type":"text","text":text})] });
    }
    let mut result = Vec::new();
    for part in content.as_array().ok_or_else(|| anyhow!(FAILURE))? {
        match field(part, "type")? {
            "text" => result.push(json!({"type":"text","text":field(part,"text")?})),
            "image_url" => {
                let url = field(&part["image_url"], "url")?;
                let source = if let Some(data) = url.strip_prefix("data:") {
                    let (mime, data) = data.split_once(";base64,").ok_or_else(|| anyhow!("Invalid image data"))?;
                    if !matches!(mime, "image/png" | "image/jpeg" | "image/gif" | "image/webp")
                        || base64::engine::general_purpose::STANDARD.decode(data).is_err()
                    {
                        bail!("Invalid image data");
                    }
                    json!({"type":"base64","media_type":mime,"data":data})
                } else {
                    let parsed = url::Url::parse(url).map_err(|_| anyhow!("Invalid image URL"))?;
                    if parsed.scheme() != "https" {
                        bail!("Unsupported image URL");
                    }
                    json!({"type":"url","url":url})
                };
                result.push(json!({"type":"image","source":source}));
            }
            _ => bail!("Unsupported Claude message content"),
        }
    }
    Ok(result)
}

pub fn messages_body(data: Value) -> Result<Value> {
    let definitions = definitions(&data)?;
    let tools: Vec<_> = definitions
        .iter()
        .map(|f| json!({"name":f["name"],"description":f["description"],"input_schema":f["parameters"]}))
        .collect();
    let mut system = Vec::new();
    let mut messages: Vec<Value> = Vec::new();
    for message in data["history"].as_array().ok_or_else(|| anyhow!("Missing conversation history"))? {
        let (role, mut content) = match field(message, "role")? {
            "system" | "developer" => {
                system.extend(parts(&message["content"])?);
                continue;
            }
            "user" => ("user", parts(&message["content"])?),
            "tool" => (
                "user",
                vec![
                    json!({"type":"tool_result","tool_use_id":field(message,"tool_call_id")?,"content":parts(&message["content"])?,"is_error":message["content"].as_str().is_some_and(|s|s.starts_with("ERROR:"))}),
                ],
            ),
            "assistant" => {
                if message.get("_responses_output").is_some() {
                    bail!("Cannot replay ChatGPT reasoning through Claude");
                }
                let canonical = calls(message)?;
                let content = if let Some(saved) = message.get("_anthropic_content") {
                    let historical: Vec<_> = canonical.iter().map(|c| json!({"name":c["function"]["name"]})).collect();
                    let replay = normalize(
                        json!({"type":"message","role":"assistant","content":saved,"stop_reason":if canonical.is_empty() {"end_turn"} else {"tool_use"}}),
                        &json!({"tools":historical}),
                    )?;
                    if calls(&replay["choices"][0]["message"])? != canonical
                        || replay["choices"][0]["message"]["content"] != message["content"]
                    {
                        bail!("Saved Claude history does not match conversation");
                    }
                    saved.as_array().ok_or_else(|| anyhow!(FAILURE))?.clone()
                } else {
                    let mut content = parts(&message["content"])?;
                    for call in canonical {
                        content.push(json!({"type":"tool_use","id":call["id"],"name":call["function"]["name"],"input":serde_json::from_str::<Value>(field(&call["function"],"arguments")?).map_err(|_|anyhow!(FAILURE))?}));
                    }
                    content
                };
                ("assistant", content)
            }
            _ => bail!("Unsupported Claude conversation role"),
        };
        if let Some(last) = messages.last_mut().filter(|last| last["role"] == role) {
            last["content"].as_array_mut().ok_or_else(|| anyhow!(FAILURE))?.append(&mut content);
        } else {
            messages.push(json!({"role":role,"content":content}));
        }
    }
    let mut body = json!({"model":field(&data,"model")?,"system":system,"messages":messages,"tools":tools,"max_tokens":data.get("max_tokens").and_then(Value::as_u64).unwrap_or(8192),"stream":true});
    if let Some(extra) = data.get("extra_body").and_then(Value::as_object) {
        for (key, value) in extra {
            if matches!(key.as_str(), "thinking" | "output_config" | "tool_choice" | "temperature" | "top_p") {
                body[key] = value.clone();
            }
        }
    }
    if data["compact"] == true {
        let object = body.as_object_mut().ok_or_else(|| anyhow!("Invalid compact request"))?;
        object.remove("tools");
        object.remove("tool_choice");
    }
    validate_body(&body)?;
    Ok(body)
}

fn validate_body(body: &Value) -> Result<()> {
    if !body.is_object()
        || field(body, "model")?.trim().is_empty()
        || field(body, "model")?.len() > 256
        || !body["messages"].is_array()
        || body["messages"].as_array().is_some_and(|a| a.len() > 4096)
        || body["max_tokens"].as_u64().is_none_or(|n| n == 0 || n > 1_000_000)
        || serde_json::to_vec(body).map_err(|_| anyhow!(FAILURE))?.len() > 16 * 1024 * 1024
    {
        bail!("Invalid Claude request");
    }
    let mut names = HashSet::new();
    if let Some(tools) = body.get("tools") {
        for tool in tools.as_array().ok_or_else(|| anyhow!(FAILURE))? {
            let name = field(tool, "name")?;
            if tool.get("type").is_some_and(|t| t != "custom")
                || name.is_empty()
                || !names.insert(name)
                || !tool["input_schema"].is_object()
            {
                bail!("Only local Claude function tools are supported");
            }
        }
    }
    if body.get("mcp_servers").is_some() {
        bail!("Hosted Claude tools are not supported");
    }
    Ok(())
}

fn normalize(response: Value, request: &Value) -> Result<Value> {
    if response["type"] != "message" || response["role"] != "assistant" {
        bail!(FAILURE);
    }
    let finish = match response["stop_reason"].as_str() {
        Some("end_turn") => "stop",
        Some("tool_use") => "tool_calls",
        _ => bail!("Claude response was incomplete; no tools were executed"),
    };
    let blocks = response["content"].as_array().ok_or_else(|| anyhow!(FAILURE))?;
    let allowed: HashSet<_> =
        request["tools"].as_array().into_iter().flatten().filter_map(|t| t["name"].as_str()).collect();
    let mut text = Vec::new();
    let mut result = Vec::new();
    let mut ids = HashSet::new();
    for block in blocks {
        match field(block, "type")? {
            "text" => text.push(field(block, "text")?),
            "tool_use" => {
                let id = field(block, "id")?;
                let name = field(block, "name")?;
                if id.is_empty() || !ids.insert(id) || !allowed.contains(name) || !block["input"].is_object() {
                    bail!("Claude returned an invalid local tool call");
                }
                result.push(
                    json!({"id":id,"type":"function","function":{"name":name,"arguments":block["input"].to_string()}}),
                );
            }
            "thinking" => {
                field(block, "thinking")?;
                if field(block, "signature")?.is_empty() {
                    bail!("Missing Claude reasoning signature");
                }
            }
            "redacted_thinking" => {
                field(block, "data")?;
            }
            _ => bail!("Unsupported Claude output block"),
        }
    }
    if (finish == "tool_calls") == result.is_empty() {
        bail!("Claude returned an incomplete tool batch");
    }
    Ok(
        json!({"id":response.get("id"),"object":"chat.completion","choices":[{"index":0,"finish_reason":finish,"message":{"role":"assistant","content":text.join("\n"),"tool_calls":result,"_anthropic_content":blocks}}],"usage":response.get("usage")}),
    )
}

fn parse_sse(bytes: &[u8], request: &Value) -> Result<Value> {
    let text = std::str::from_utf8(bytes).map_err(|_| anyhow!(FAILURE))?.replace("\r\n", "\n");
    if bytes.len() > LIMIT || !text.ends_with("\n\n") {
        bail!("Claude response stream was interrupted");
    }
    let mut message: Option<Value> = None;
    let mut blocks: BTreeMap<u64, Value> = BTreeMap::new();
    let mut open = HashSet::new();
    let mut arguments: BTreeMap<u64, String> = BTreeMap::new();
    let mut stopped = false;
    let mut finish = false;
    for frame in text.split("\n\n") {
        let data = frame
            .lines()
            .filter_map(|l| l.strip_prefix("data:").map(|s| s.strip_prefix(' ').unwrap_or(s)))
            .collect::<Vec<_>>()
            .join("\n");
        if data.is_empty() {
            continue;
        }
        let event: Value = serde_json::from_str(&data).map_err(|_| anyhow!(FAILURE))?;
        let kind = field(&event, "type")?;
        if kind == "ping" {
            continue;
        }
        if stopped || kind == "error" || event.get("error").is_some_and(|v| !v.is_null()) {
            bail!(FAILURE);
        }
        match kind {
            "message_start" => {
                if message.is_some()
                    || event["message"]["type"] != "message"
                    || event["message"]["role"] != "assistant"
                    || field(&event["message"], "id")?.is_empty()
                {
                    bail!(FAILURE);
                }
                message = Some(event["message"].clone());
            }
            "content_block_start" => {
                let index = event["index"].as_u64().ok_or_else(|| anyhow!(FAILURE))?;
                if message.is_none() || blocks.contains_key(&index) || index != blocks.len() as u64 || finish {
                    bail!(FAILURE);
                }
                blocks.insert(index, event["content_block"].clone());
                open.insert(index);
            }
            "content_block_delta" => {
                let index = event["index"].as_u64().ok_or_else(|| anyhow!(FAILURE))?;
                if !open.contains(&index) || finish {
                    bail!(FAILURE);
                }
                let block = blocks.get_mut(&index).ok_or_else(|| anyhow!(FAILURE))?;
                let delta = &event["delta"];
                match field(delta, "type")? {
                    "text_delta" if block["type"] == "text" => {
                        let mut text = field(block, "text")?.to_owned();
                        text.push_str(field(delta, "text")?);
                        block["text"] = json!(text);
                    }
                    "input_json_delta" if block["type"] == "tool_use" => {
                        arguments.entry(index).or_default().push_str(field(delta, "partial_json")?)
                    }
                    "thinking_delta" if block["type"] == "thinking" => {
                        let mut text = field(block, "thinking")?.to_owned();
                        text.push_str(field(delta, "thinking")?);
                        block["thinking"] = json!(text);
                    }
                    "signature_delta" if block["type"] == "thinking" => {
                        let mut signature = block["signature"].as_str().unwrap_or("").to_owned();
                        signature.push_str(field(delta, "signature")?);
                        block["signature"] = json!(signature);
                    }
                    _ => bail!("Unsupported Claude stream delta"),
                }
            }
            "content_block_stop" => {
                let index = event["index"].as_u64().ok_or_else(|| anyhow!(FAILURE))?;
                if !open.remove(&index) {
                    bail!(FAILURE);
                }
                if let Some(json) = arguments.remove(&index) {
                    blocks.get_mut(&index).ok_or_else(|| anyhow!(FAILURE))?["input"] =
                        serde_json::from_str(&json).map_err(|_| anyhow!(FAILURE))?;
                }
            }
            "message_delta" => {
                if message.is_none() || !open.is_empty() || finish {
                    bail!(FAILURE);
                }
                let m = message.as_mut().ok_or_else(|| anyhow!(FAILURE))?;
                m["stop_reason"] = event["delta"]["stop_reason"].clone();
                if let Some(usage) = event.get("usage").and_then(Value::as_object) {
                    for (k, v) in usage {
                        m["usage"][k] = v.clone();
                    }
                }
                finish = true;
            }
            "message_stop" => {
                if !finish || !open.is_empty() {
                    bail!(FAILURE);
                }
                stopped = true;
            }
            _ => bail!("Unsupported Claude stream event"),
        }
    }
    if !stopped {
        bail!("Claude response stream ended without completion");
    }
    let mut message = message.ok_or_else(|| anyhow!(FAILURE))?;
    message["content"] = json!(blocks.into_values().collect::<Vec<_>>());
    normalize(message, request)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn logout_clears_local_credentials_before_slow_cli_and_survives_cancellation() {
        let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
        let auth = ClaudeAuth::at(dir.path().join("claude")).unwrap();
        let credentials = auth.root.join("ant/credentials/i.json");
        fs::create_dir_all(credentials.parent().unwrap()).unwrap();
        fs::write(&credentials, b"synthetic-credentials-not-a-token").unwrap();
        let reached = std::sync::Arc::new(tokio::sync::Notify::new());
        let pending = reached.clone();
        let running = auth.clone();
        let task = tokio::spawn(async move {
            running
                .logout_using(None, Duration::from_secs(20), || async move {
                    pending.notify_one();
                    std::future::pending::<bool>().await
                })
                .await
        });
        reached.notified().await;
        assert!(auth.root.join("signed-out").is_file());
        assert!(!credentials.exists());
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        let reopened = ClaudeAuth::at(auth.root.clone()).unwrap();
        assert!(reopened.token_locked().await.is_err());
        assert_eq!(reopened.status().await.unwrap()["active"], Value::Null);
    }

    #[tokio::test]
    async fn logout_cli_timeout_reports_unconfirmed_and_keeps_signed_out_marker() {
        let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
        let auth = ClaudeAuth::at(dir.path().join("claude")).unwrap();
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            auth.logout_using(None, Duration::from_millis(5), std::future::pending::<bool>),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(result["signed_out"], true);
        assert_eq!(result["official_cli_logout_completed"], false);
        assert_eq!(result["remote_revocation_confirmed"], false);
        assert!(auth.root.join("signed-out").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_child_directories_without_changing_target_permissions() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
        for name in ["ant", "home"] {
            let root = dir.path().join(format!("root-{name}"));
            fs::create_dir(&root).unwrap();
            let target = dir.path().join(format!("unrelated-{name}"));
            fs::create_dir(&target).unwrap();
            fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).unwrap();
            symlink(&target, root.join(name)).unwrap();
            assert!(ClaudeAuth::at(root).is_err());
            assert_eq!(fs::metadata(target).unwrap().permissions().mode() & 0o777, 0o755);
        }
    }

    #[test]
    fn messages_convert_system_tools_and_base64_images() {
        let body = messages_body(json!({"model":"claude-test","max_tokens":500,"history":[{"role":"system","content":"rules"},{"role":"user","content":[{"type":"image_url","image_url":{"url":"data:image/png;base64,aA=="}}]},{"role":"assistant","content":"","tool_calls":[{"id":"c","function":{"name":"read","arguments":"{}"}}]},{"role":"tool","tool_call_id":"c","content":"result"}],"tools":[{"function":{"name":"read","parameters":{"type":"object"}}}]})).unwrap();
        assert_eq!(body["system"][0]["text"], "rules");
        assert_eq!(body["messages"][0]["content"][0]["source"]["type"], "base64");
        assert_eq!(body["messages"][2]["content"][0]["type"], "tool_result");
        assert_eq!(body["tools"][0]["name"], "read");
    }

    #[test]
    fn complete_messages_validate_terminal_tool_batch() {
        let request = json!({"tools":[{"name":"read"}]});
        let response = json!({"type":"message","role":"assistant","content":[{"type":"tool_use","id":"c","name":"read","input":{}}],"stop_reason":"tool_use"});
        assert_eq!(normalize(response.clone(), &request).unwrap()["choices"][0]["finish_reason"], "tool_calls");
        let mut truncated = response.clone();
        truncated["stop_reason"] = json!("max_tokens");
        assert!(normalize(truncated, &request).is_err());
        let mut duplicate = response;
        let block = duplicate["content"][0].clone();
        duplicate["content"].as_array_mut().unwrap().push(block);
        assert!(normalize(duplicate, &request).is_err());
    }

    fn event(value: Value) -> String {
        format!("event: {}\ndata: {}\n\n", value["type"].as_str().unwrap(), value)
    }

    #[test]
    fn stream_reconstructs_tools_and_requires_terminal_message_stop() {
        let request = json!({"tools":[{"name":"read"}]});
        let events = [
            json!({"type":"message_start","message":{"id":"m","type":"message","role":"assistant","content":[],"usage":{"input_tokens":1}}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"c","name":"read","input":{}}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"path\":\"x\"}"}}),
            json!({"type":"content_block_stop","index":0}),
            json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":2}}),
            json!({"type":"message_stop"}),
        ];
        let fixture = events.iter().cloned().map(event).collect::<String>();
        let result = parse_sse(fixture.as_bytes(), &request).unwrap();
        assert_eq!(result["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"], "{\"path\":\"x\"}");
        assert_eq!(result["usage"]["output_tokens"], 2);
        let truncated = events[..events.len() - 1].iter().cloned().map(event).collect::<String>();
        assert!(parse_sse(truncated.as_bytes(), &request).is_err());
        assert!(
            parse_sse(format!("{}{}", fixture, event(json!({"type":"message_stop"}))).as_bytes(), &request).is_err()
        );
        assert!(parse_sse(fixture.replace("\"tool_use\"}", "\"max_tokens\"}").as_bytes(), &request).is_err());
    }

    #[test]
    fn thinking_signatures_are_preserved_and_foreign_replay_is_rejected() {
        let blocks = json!([{"type":"thinking","thinking":"private thought","signature":"signed"},{"type":"text","text":"answer"}]);
        let result = normalize(
            json!({"type":"message","role":"assistant","content":blocks,"stop_reason":"end_turn"}),
            &json!({"tools":[]}),
        )
        .unwrap();
        assert_eq!(result["choices"][0]["message"]["_anthropic_content"][0]["signature"], "signed");
        let body =
            messages_body(json!({"model":"test","history":[result["choices"][0]["message"].clone()],"tools":[]}))
                .unwrap();
        assert_eq!(body["messages"][0]["content"], blocks);
        assert!(messages_body(
            json!({"model":"test","history":[{"role":"assistant","content":"x","_responses_output":[]}]})
        )
        .is_err());
    }

    #[test]
    fn ant_command_never_inherits_credential_or_endpoint_overrides() {
        let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
        let auth = ClaudeAuth::at(dir.path().join("claude")).unwrap();
        let command = auth.command(&["auth", "print-credentials", "--access-token"]);
        let env: std::collections::HashMap<_, _> = command
            .as_std()
            .get_envs()
            .filter_map(|(k, v)| v.map(|v| (k.to_string_lossy().to_string(), v.to_string_lossy().to_string())))
            .collect();
        assert!(!env.contains_key("ANTHROPIC_API_KEY"));
        assert!(!env.contains_key("ANTHROPIC_AUTH_TOKEN"));
        assert_eq!(env["ANTHROPIC_CONFIG_DIR"], auth.root.join("ant").to_string_lossy());
        assert_eq!(env["HOME"], auth.root.join("home").to_string_lossy());
        let arguments = command.as_std().get_args().map(|a| a.to_string_lossy().to_string()).collect::<Vec<_>>();
        assert_eq!(arguments, vec!["auth", "print-credentials", "--access-token", "--profile", "i"]);
    }
}
