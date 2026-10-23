//! Explicit provider Files transport. Network execution and credentials stay in the host.
#[path = "provider_file_api.rs"]
mod file_api;
#[path = "provider_lifecycle.rs"]
mod lifecycle;
#[path = "provider_manage.rs"]
mod manage;
#[cfg(test)]
#[path = "provider_files_tests.rs"]
mod tests;
use super::{configured, context::digest, paths::AppPaths, provider, state};
use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

fn file_id(value: &Value) -> Result<&str> {
    let id = super::text(value, "id")?;
    ensure!(
        (id.starts_with("file-") || id.starts_with("file_"))
            && id.len() > 5
            && id.len() <= 128
            && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "invalid Files id"
    );
    Ok(id)
}
fn now() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}
fn image(value: &Value) -> Result<(Vec<u8>, &str)> {
    ensure!(
        value.as_object().is_some_and(|o| o.keys().all(|k| matches!(k.as_str(), "type" | "image_url")))
            && value["image_url"].as_object().is_some_and(|o| o.keys().all(|k| k == "url")),
        "Files mode cannot silently discard image options; use base64 for explicit detail/options"
    );
    let url = value["image_url"]["url"].as_str().context("Files requires image data URLs")?;
    let (mime, encoded) =
        url.strip_prefix("data:").and_then(|s| s.split_once(";base64,")).context("Files requires image data URLs")?;
    ensure!(encoded.len() <= 12 * 1024 * 1024, "image too large");
    let bytes = STANDARD.decode(encoded)?;
    ensure!(bytes.len() <= 8 * 1024 * 1024, "image exceeds 8 MiB");
    let valid = match mime {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(b"\xff\xd8\xff"),
        "image/gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "image/webp" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
        _ => false,
    };
    ensure!(valid, "invalid image signature/type");
    Ok((bytes, mime))
}
fn commit(path: &Path, db: &Value) -> Result<()> {
    state::atomic(path, db)?;
    #[cfg(unix)]
    fs::File::open(path.parent().context("missing registry parent")?)?.sync_all()?;
    Ok(())
}
pub(crate) fn registry(root: &Path, args: &[String]) -> Result<Value> {
    fs::create_dir_all(root)?;
    let path = state::workspace_state(root, "provider-files.json")?;
    let _lock = state::Lock::acquire(&root.join("provider-files.lock"))?;
    let mut db: Value = if path.try_exists()? {
        serde_json::from_slice(&state::read_bounded(&path, 2 * 1024 * 1024)?)?
    } else {
        json!({"version":1,"entries":{}})
    };
    ensure!(db["version"] == 1 && db["entries"].is_object(), "invalid Files registry");
    let forget = super::option(args, "--forget");
    let retired = super::option(args, "--forget-retired");
    ensure!(forget.is_none() || retired.is_none(), "choose --forget or --forget-retired");
    let collection = if retired.is_some() { "retired" } else { "entries" };
    if let Some(key) = forget.or(retired) {
        ensure!(args.iter().any(|a| a == "--confirm"), "forget requires --confirm; it does NOT delete the remote file");
        let record = db[collection].get(&key).context("unknown Files registry key")?;
        ensure!(
            matches!(record["status"].as_str(), Some("ready" | "deleted" | "retired"))
                || args.iter().any(|a| a == "--ack-unknown"),
            "pending upload outcome unknown; inspect remote files first, then explicitly --ack-unknown (may duplicate upload)"
        );
        db[collection].as_object_mut().unwrap().remove(&key);
        commit(&path, &db)?;
    }
    Ok(db)
}
fn multipart_for(data: &Value, bytes: &[u8], mime: &str, ttl: Option<u64>, filename: &str) -> (String, Vec<u8>) {
    let boundary = format!("i-{}", uuid::Uuid::new_v4());
    let mut body = Vec::new();
    let mut field = |name: &str, value: &str| {
        body.extend_from_slice(
            format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes(),
        )
    };
    if data["protocol"] != "anthropic" {
        field("purpose", file_api::purpose(data));
    }
    if let Some(ttl) = ttl {
        field("expires_after[anchor]", "created_at");
        field("expires_after[seconds]", &ttl.to_string());
    }
    body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: {mime}\r\n\r\n").as_bytes());
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}
pub(crate) fn handle(
    event: &str,
    mut data: Value,
    args: &[String],
    paths: &AppPaths,
    workspace: &str,
    id: Option<&str>,
) -> Result<Value> {
    if event == "provider_manage" {
        return manage::handle(data, args, paths, workspace, id);
    }
    if event != "provider_prepare" {
        return provider::handle(event, data);
    }
    ensure!(data["version"] == 1, "unsupported provider contract");
    let (config, _) = configured(args)?;
    file_api::validate(&config)?;
    ensure!(
        config["auto_gc"]["enabled"] != true || data["allow_cleanup"] == true,
        "auto_gc requires separate host provider_cleanup=true permission"
    );
    if data.get("operation_result").is_some() {
        ensure!(
            data["operation_state"]["config"] == digest(&config),
            "provider configuration changed during upload; reconcile pending state"
        );
    }
    match config.get("images").and_then(Value::as_str).unwrap_or("base64") {
        "base64" => {
            ensure!(config.get("images").is_none_or(Value::is_string), "invalid images mode");
            return provider::handle(event, data);
        }
        "files" => (),
        _ => bail!("unknown provider images mode"),
    }
    file_api::supported(&data)?;
    ensure!(data["protocol"] != "openai", "OpenAI image Files requires protocol=responses, not Chat Completions");
    ensure!(data["allow_operations"] == true, "Files requires explicit host provider_operations=true");
    let account = config["account_scope"]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= 128)
        .context("Files requires account_scope")?;
    let ttl = match config.get("expires_after_seconds") {
        None => Some(3600),
        Some(Value::Null) => None,
        Some(value) => {
            Some(value.as_u64().filter(|n| (3600..=2592000).contains(n)).context("invalid Files expiration")?)
        }
    };
    ensure!(
        data["protocol"] != "anthropic" || (config.get("expires_after_seconds") == Some(&Value::Null)),
        "Anthropic beta Files has no server TTL; explicitly set expires_after_seconds:null"
    );
    provider::handle(event, data.clone())?; // Reject invalid model options before scheduling any upload.
    for message in data["history"].as_array().context("missing history")? {
        if let Some(parts) = message["content"].as_array() {
            for part in parts.iter().filter(|p| p["type"] == "image_url") {
                image(part)?;
            }
        }
    }
    let root = paths.workspace(Path::new(workspace), id)?.1;
    data["_files_policy"] = config.clone();
    lifecycle::finish(&mut data, account, &root)?;
    let mut plan = files(data.clone(), account, ttl, &root)?;
    if plan.get("operation").is_none()
        && let Some(cleanup) = lifecycle::next(&data, &config, account, &root)?
    {
        plan = cleanup;
    }
    if plan.get("operation").is_some() {
        plan["state"]["config"] = json!(digest(&config));
    }
    Ok(plan)
}
fn endpoint(data: &Value) -> Result<String> {
    let mut endpoint = url::Url::parse(super::text(data, "base_url")?)?;
    ensure!(
        matches!(endpoint.scheme(), "http" | "https")
            && endpoint.username().is_empty()
            && endpoint.password().is_none()
            && endpoint.query().is_none()
            && endpoint.fragment().is_none(),
        "Files base URL must be an HTTP(S) directory without credentials/query/fragment"
    );
    let base = endpoint.path().trim_end_matches('/');
    let path = match data["protocol"].as_str() {
        Some("anthropic") => format!("{}/v1/files", base.strip_suffix("/v1").unwrap_or(base)),
        Some("responses" | "openai") => format!("{base}/files"),
        _ => format!("{}/files", base.strip_suffix("/v1").unwrap_or(base)),
    };
    endpoint.set_path(&path);
    Ok(endpoint.to_string())
}
fn files(mut data: Value, account: &str, ttl: Option<u64>, root: &Path) -> Result<Value> {
    let endpoint = endpoint(&data)?;
    let scope = file_api::scope(&data, &endpoint, account)?;
    let policy = data["_files_policy"].clone();
    let path = state::workspace_state(root, "provider-files.json")?;
    fs::create_dir_all(root)?;
    let _lock = state::Lock::acquire(&root.join("provider-files.lock"))?;
    let mut db: Value = if path.try_exists()? {
        serde_json::from_slice(&state::read_bounded(&path, 2 * 1024 * 1024)?)?
    } else {
        json!({"version":1,"entries":{}})
    };
    ensure!(db["version"] == 1 && db["entries"].is_object(), "invalid Files registry");
    let timestamp = now()?;
    if data.get("operation_result").is_some() {
        let key = super::text(&data["operation_state"], "key")?;
        let nonce = super::text(&data["operation_state"], "nonce")?;
        let record = &db["entries"][key];
        ensure!(
            record["scope"] == scope && record["status"] == "pending" && record["nonce"] == nonce,
            "Files operation state mismatch"
        );
        let result = &data["operation_result"];
        let id = file_id(result)?;
        let expiry = match result.get("expires_at") {
            None | Some(Value::Null) if record["ttl"].is_null() => Value::Null,
            Some(value) => {
                let expires = value.as_u64().context("invalid Files expires_at")?;
                ensure!(expires > timestamp + 60, "uploaded file already expired");
                if let Some(ttl) = record["ttl"].as_u64() {
                    ensure!(expires <= timestamp + ttl + 60, "Files expiration exceeds requested TTL");
                }
                json!(expires)
            }
            _ => bail!("Files response missing expires_at; outcome unknown"),
        };
        db["entries"][key] = json!({"scope":scope,"status":"ready","id":id,"expires_at":expiry,
                "filename":record["filename"],"bytes":record["bytes"],"created_at":record["created_at"],"owned":true});
        commit(&path, &db)?;
    }
    if data["allow_cleanup"] == true && lifecycle::prune_ready(&mut db, &scope, &policy, timestamp) {
        commit(&path, &db)?;
    }
    let remaining = data["operations_remaining"].as_u64();
    let wire = json!({"protocol":data["protocol"]});
    for message in data["history"].as_array_mut().context("missing history")? {
        let Some(parts) = message["content"].as_array_mut() else { continue };
        for part in parts {
            if part["type"] != "image_url" {
                continue;
            }
            let (bytes, mime) = image(part)?;
            let hash = format!("{:x}", Sha256::digest(&bytes));
            let key = digest(&json!([scope, hash]));
            if let Some(record) = db["entries"].get(&key).cloned() {
                ensure!(
                    record["status"] != "deleted",
                    "Files reference deleted; explicitly forget registry entry {key} to permit re-upload"
                );
                ensure!(
                    record["scope"] == scope && record["status"] == "ready",
                    "Files reference pending/unknown; reconcile registry entry {key} before retrying"
                );
                ensure!(record.get("expires_at").is_some(), "Files registry missing expiry");
                file_id(&record)?;
                if !lifecycle::refresh(&record, &policy, timestamp)? {
                    let id = file_id(&record)?;
                    *part = json!({"type":"file","file_id":id});
                    continue;
                }
                ensure!(remaining != Some(0), "Files refresh operation limit exhausted");
                lifecycle::retire(&mut db, &record, timestamp)?;
            }
            ensure!(
                db["entries"].as_object().unwrap().len() < 1000 || db["entries"].get(&key).is_some(),
                "Files registry full; explicit cleanup required"
            );
            ensure!(
                remaining != Some(0),
                "Files upload operation limit exhausted; continue explicitly after reviewing completed uploads"
            );
            let nonce = uuid::Uuid::new_v4().to_string();
            let filename = format!("image-{hash}.{}", mime.strip_prefix("image/").context("invalid image MIME")?);
            let (content_type, body) = multipart_for(&wire, &bytes, mime, ttl, &filename);
            db["entries"][&key] = json!({"scope":scope,"status":"pending","nonce":nonce,"created_at":timestamp,"ttl":ttl,
                    "filename":filename,"bytes":bytes.len()});
            commit(&path, &db)?;
            let mut operation = file_api::operation(&wire, &endpoint, "POST");
            operation["content_type"] = json!(content_type);
            operation["body_base64"] = json!(STANDARD.encode(body));
            return Ok(json!({"version":1,"state":{"key":key,"nonce":nonce},"operation":operation}));
        }
    }
    let anthropic = data["protocol"] == "anthropic";
    let mut plan = provider::handle("provider_prepare", data)?;
    if anthropic {
        plan["headers"]["anthropic-beta"] = json!("files-api-2025-04-14");
    }
    Ok(plan)
}
