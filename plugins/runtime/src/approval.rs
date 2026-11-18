//! Optional approval policy. The core owns the final gate and never trusts an invalid decision.
use super::{configured, option, state};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::path::Path;

pub(crate) fn set_mode(path: &Path, mode: &str) -> Result<()> {
    ensure!(matches!(mode, "ask" | "auto"), "mode must be ask or auto");
    state::atomic(path, &json!({"mode":mode}))
}
pub(crate) fn mode(path: &Path) -> Result<String> {
    let value: Value = serde_json::from_slice(&state::read_bounded(path, 4096)?)?;
    let mode = value["mode"].as_str().context("missing approval mode")?;
    ensure!(matches!(mode, "ask" | "auto"), "mode must be ask or auto");
    Ok(mode.into())
}
pub(crate) fn handle(event: &str, mut data: Value, args: &[String]) -> Result<Value> {
    if event != "tool_approval" {
        return Ok(data);
    }
    let decision = data["decision"].as_str().context("missing approval decision")?;
    ensure!(matches!(decision, "allow" | "ask" | "deny"), "invalid approval decision");
    if decision == "deny" {
        return Ok(data);
    }
    for flag in ["--config", "--mode-file"] {
        ensure!(!args.iter().any(|arg| arg == flag) || option(args, flag).is_some(), "{flag} requires a value");
    }
    let (config, _) = configured(args)?;
    let fields = config.as_object().context("approval config must be an object")?;
    ensure!(fields.keys().all(|key| matches!(key.as_str(), "mode" | "deny_tools")), "unknown approval config field");
    if let Some(denied) = config.get("deny_tools") {
        let denied = denied.as_array().context("deny_tools must be an array")?;
        ensure!(denied.iter().all(Value::is_string), "deny_tools must contain strings");
        if denied.contains(&data["name"]) {
            data["decision"] = json!("deny");
            return Ok(data);
        }
    }
    let selected = match option(args, "--mode-file") {
        Some(path) => Some(mode(Path::new(&path))?),
        None => config.get("mode").map(|v| v.as_str().map(str::to_owned).context("invalid mode")).transpose()?,
    };
    if let Some(mode) = selected {
        ensure!(matches!(mode.as_str(), "ask" | "auto"), "mode must be ask or auto");
        let read = data["external"] == false
            && matches!(data["name"].as_str(), Some("read" | "view_image" | "list" | "glob" | "search"));
        data["decision"] = json!(if mode == "auto" || read { "allow" } else { "ask" });
    }
    Ok(data)
}
