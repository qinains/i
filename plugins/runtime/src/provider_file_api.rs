//! Provider-specific Files wire descriptions; never performs HTTP.
use super::*;
pub(super) fn supported(data: &Value) -> Result<&str> {
    let kind = super::super::text(data, "protocol")?;
    ensure!(matches!(kind, "deepseek" | "responses" | "openai" | "anthropic"), "unsupported Files provider");
    Ok(kind)
}
pub(super) fn scope(data: &Value, endpoint: &str, account: &str) -> Result<String> {
    let key = super::super::text(data, "credential_scope")?;
    Ok(if data["protocol"] == "deepseek" {
        digest(&json!([endpoint, account, key]))
    } else {
        digest(&json!([endpoint, account, key, data["protocol"]]))
    })
}
pub(super) fn headers(data: &Value) -> Value {
    if data["protocol"] == "anthropic" {
        json!({"anthropic-version":"2023-06-01","anthropic-beta":"files-api-2025-04-14"})
    } else {
        json!({})
    }
}
pub(super) fn operation(data: &Value, url: &str, method: &str) -> Value {
    json!({"url":url,"method":method,"body_base64":"","headers":headers(data),
        "auth":if data["protocol"]=="anthropic" {json!({"header":"x-api-key","prefix":""})}
            else {json!({"header":"authorization","prefix":"Bearer "})}})
}
pub(super) fn purpose(data: &Value) -> &'static str {
    if data["protocol"] == "deepseek" { "user_data" } else { "vision" }
}
pub(super) fn metadata(data: &Value, value: &Value) -> Result<Value> {
    file_id(value)?;
    let mut v = value.clone();
    if data["protocol"] == "anthropic" {
        ensure!(
            v["type"] == "file" && v["size_bytes"].is_u64() && v["created_at"].is_string(),
            "invalid Anthropic Files metadata"
        );
        v["bytes"] = v["size_bytes"].clone();
        v["object"] = json!("file");
        v["purpose"] = json!("vision");
    } else {
        ensure!(v["created_at"].is_u64(), "invalid Files creation time");
    }
    ensure!(
        v["object"] == "file" && v["bytes"].is_u64() && v["filename"].is_string() && v["purpose"] == purpose(data),
        "invalid Files metadata"
    );
    ensure!(v.get("expires_at").is_none_or(|v| v.is_null() || v.is_u64()), "invalid Files expiration");
    Ok(v)
}
pub(super) fn deleted(data: &Value, value: &Value, id: &str) -> Result<()> {
    ensure!(
        value["id"] == id
            && if data["protocol"] == "anthropic" {
                value["type"] == "file_deleted"
            } else {
                value["object"] == "file" && value["deleted"] == true
            },
        "delete not confirmed; outcome unknown"
    );
    Ok(())
}
pub(super) fn validate(config: &Value) -> Result<()> {
    ensure!(
        config.as_object().is_some_and(|o| o.keys().all(|k| matches!(
            k.as_str(),
            "images" | "account_scope" | "expires_after_seconds" | "auto_refresh" | "auto_gc"
        ))),
        "unknown provider configuration field"
    );
    for name in ["auto_refresh", "auto_gc"] {
        if let Some(v) = config.get(name) {
            ensure!(
                v.as_object().is_some_and(|o| o
                    .keys()
                    .all(|k| matches!(k.as_str(), "enabled" | "max_age_seconds" | "grace_seconds"))),
                "invalid Files lifecycle policy"
            );
            ensure!(v["enabled"].is_boolean(), "lifecycle enabled must be boolean");
            for field in ["max_age_seconds", "grace_seconds"] {
                if let Some(n) = v.get(field) {
                    ensure!(
                        n.as_u64().is_some_and(|n| (3600..=2592000).contains(&n)),
                        "lifecycle duration must be 3600..2592000"
                    );
                }
            }
            ensure!(
                v.get(if name == "auto_gc" { "max_age_seconds" } else { "grace_seconds" }).is_none(),
                "wrong lifecycle duration field"
            );
        }
    }
    Ok(())
}
