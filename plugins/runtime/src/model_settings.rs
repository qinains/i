//! Host-owned model settings. No credential export or generation requests.
use super::{console::without, option, paths::AppPaths, state};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
fn file(paths: &AppPaths, backend: &[String]) -> PathBuf {
    option(backend, "--config").map(PathBuf::from).unwrap_or_else(|| paths.config_dir.join("config.json"))
}
pub(super) fn current(paths: &AppPaths, backend: &[String]) -> Result<Value> {
    let file = file(paths, backend);
    let value =
        if file.try_exists()? { serde_json::from_slice(&state::read_bounded(&file, 1_000_000)?)? } else { json!({}) };
    ensure!(value.is_object(), "Invalid model configuration");
    Ok(value)
}
fn edit(config: &mut Value, model: Option<&str>, effort: Option<&str>) -> Result<()> {
    if let Some(model) = model {
        ensure!(!model.is_empty() && model.len() <= 256 && !model.chars().any(char::is_control), "Invalid model ID");
    }
    let effort = effort.unwrap_or("auto");
    ensure!(
        ["auto", "default", "none", "minimal", "low", "medium", "high", "xhigh", "max"].contains(&effort),
        "Unknown effort; use auto, none, minimal, low, medium, high, xhigh or max (availability depends on model)"
    );
    let protocol = config["protocol"].as_str().unwrap_or("openai").to_owned();
    ensure!(
        ["openai", "responses", "deepseek", "anthropic"].contains(&protocol.as_str()),
        "Configure a supported provider protocol first"
    );
    if let Some(model) = model {
        config["model"] = json!(model);
    }
    if config.get("extra_body").is_none() {
        config["extra_body"] = json!({});
    }
    ensure!(config["extra_body"].is_object(), "extra_body must be an object");
    let extra = config["extra_body"].as_object_mut().unwrap();
    let auto = matches!(effort, "auto" | "default");
    match protocol.as_str() {
        "responses" => {
            let reasoning = extra.entry("reasoning").or_insert(json!({}));
            let object = reasoning.as_object_mut().context("reasoning must be an object")?;
            if auto {
                object.remove("effort");
            } else {
                object.insert("effort".into(), json!(effort));
            }
            if object.is_empty() {
                extra.remove("reasoning");
            }
        }
        "anthropic" => {
            ensure!(!matches!(effort, "minimal"), "Claude does not accept minimal effort; use low or auto");
            let output = extra.entry("output_config").or_insert(json!({}));
            let output = output.as_object_mut().context("output_config must be an object")?;
            if auto || effort == "none" {
                output.remove("effort");
            } else {
                output.insert("effort".into(), json!(effort));
            }
            if output.is_empty() {
                extra.remove("output_config");
            }
            if auto {
                extra.remove("thinking");
            } else {
                extra.insert("thinking".into(), json!({"type":if effort=="none" {"disabled"}else{"adaptive"}}));
            }
        }
        _ => {
            if auto {
                extra.remove("reasoning_effort");
            } else {
                extra.insert("reasoning_effort".into(), json!(effort));
            }
        }
    }
    Ok(())
}
pub(super) fn status(paths: &AppPaths, backend: &[String]) -> Result<String> {
    let value = current(paths, backend)?;
    let effort = value["extra_body"]["reasoning"]["effort"]
        .as_str()
        .or_else(|| value["extra_body"]["reasoning_effort"].as_str())
        .or_else(|| value["extra_body"]["output_config"]["effort"].as_str())
        .unwrap_or(if value["extra_body"]["thinking"]["type"] == "disabled" { "none" } else { "auto" });
    Ok(format!(
        "Model: {}\nThinking effort: {effort}\n/model MODEL [EFFORT] | /model effort LEVEL | /model status\nEffort values are provider/model-dependent; auto uses the provider default. No model request is sent by these commands.",
        option(backend, "--model")
            .or_else(|| option(backend, "-m"))
            .or_else(|| value["model"].as_str().map(str::to_owned))
            .unwrap_or_else(|| "(not configured)".into())
    ))
}
pub(super) fn change(
    paths: &AppPaths,
    backend: &mut Vec<String>,
    model: Option<&str>,
    effort: Option<&str>,
) -> Result<String> {
    let file = file(paths, backend);
    fs::create_dir_all(
        file.parent().filter(|path| !path.as_os_str().is_empty()).unwrap_or_else(|| std::path::Path::new(".")),
    )?;
    ensure!(!file.is_symlink(), "Configuration cannot be a symlink");
    let _guard = state::Lock::acquire(&file.with_extension("lock"))?;
    let mut value = current(paths, backend)?;
    edit(&mut value, model, effort)?;
    if file.try_exists()? {
        state::atomic(
            &file.with_extension(format!("{}.backup.json", uuid::Uuid::new_v4())),
            &current(paths, backend)?,
        )?;
    }
    state::atomic(&file, &value)?;
    if model.is_some() {
        *backend = without(&without(backend, "--model"), "-m");
    }
    Ok(format!("Saved model settings; applies to your next task.\n{}", status(paths, backend)?))
}
pub(super) fn command(args: &[&str], paths: &AppPaths, backend: &mut Vec<String>) -> Result<String> {
    match args {
        ["/model", "status"] => status(paths, backend),
        ["/model", "effort" | "thinking", effort] => change(paths, backend, None, Some(effort)),
        ["/model", model] => change(paths, backend, Some(model), None),
        ["/model", model, effort] => change(paths, backend, Some(model), Some(effort)),
        _ => bail!("Usage: /model [list|status|MODEL [EFFORT]|effort LEVEL]"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changes_are_atomic_backed_up_and_invalid_effort_preserves_config() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths {
            config_dir: root.path().join("config"),
            data_dir: root.path().join("data"),
            cache_dir: root.path().join("cache"),
            runtime_dir: root.path().join("runtime"),
        };
        fs::create_dir_all(&paths.config_dir).unwrap();
        let original = json!({"model":"old","protocol":"responses","provider":{"managed_connection":"chatgpt"},"max_tokens":1234,"api_key":"SYNTHETIC_ONLY","extra_body":{"text":{"verbosity":"low"}}});
        let file = paths.config_dir.join("config.json");
        state::atomic(&file, &original).unwrap();
        let mut backend = vec!["--model".into(), "override".into(), "-C".into(), ".".into()];
        assert!(change(&paths, &mut backend, Some("new"), Some("bogus")).is_err());
        assert_eq!(current(&paths, &backend).unwrap(), original);
        let output = change(&paths, &mut backend, Some("new"), Some("high")).unwrap();
        assert!(!output.contains("SYNTHETIC_ONLY"));
        assert_eq!(backend, vec!["-C", "."]);
        let value = current(&paths, &backend).unwrap();
        assert_eq!(value["model"], "new");
        assert_eq!(value["api_key"], "SYNTHETIC_ONLY");
        assert_eq!(value["provider"], original["provider"]);
        let backups: Vec<_> = fs::read_dir(&paths.config_dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with("backup.json"))
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(serde_json::from_slice::<Value>(&fs::read(backups[0].path()).unwrap()).unwrap(), original);
    }
    #[test]
    fn switches_model_and_effort_without_touching_connection_or_other_options() {
        let mut config = json!({"model":"old","protocol":"responses","provider":{"managed_connection":"chatgpt","exec":["private-helper"]},"max_tokens":65536,"extra_body":{"text":{"verbosity":"low"},"reasoning":{"summary":"auto"}}});
        let provider = config["provider"].clone();
        edit(&mut config, Some("new"), Some("high")).unwrap();
        assert_eq!(config["model"], "new");
        assert_eq!(config["extra_body"]["reasoning"], json!({"summary":"auto","effort":"high"}));
        assert_eq!(config["provider"], provider);
        assert_eq!(config["max_tokens"], 65536);
        edit(&mut config, Some("next"), None).unwrap();
        assert!(config["extra_body"]["reasoning"].get("effort").is_none());
        assert_eq!(config["extra_body"]["text"]["verbosity"], "low");
    }
    #[test]
    fn adapters_use_different_effort_fields_and_reject_invalid_input() {
        for (protocol, key) in [("openai", "reasoning_effort"), ("deepseek", "reasoning_effort")] {
            let mut config = json!({"protocol":protocol});
            edit(&mut config, None, Some("low")).unwrap();
            assert_eq!(config["extra_body"][key], "low");
        }
        let mut config =
            json!({"protocol":"anthropic","extra_body":{"output_config":{"format":{"type":"json_schema"}}}});
        edit(&mut config, None, Some("high")).unwrap();
        assert_eq!(config["extra_body"]["output_config"]["effort"], "high");
        assert_eq!(config["extra_body"]["thinking"]["type"], "adaptive");
        assert!(config["extra_body"]["output_config"].get("format").is_some());
        assert!(edit(&mut config, None, Some("bogus")).is_err());
        assert!(edit(&mut config, Some("bad\nmodel"), None).is_err());
    }
}
