//! Explicit analysis policy only; transcript projection and model execution belong to the host.
use super::{configured, state, text};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};

const SPEC: &str = include_str!("../../reflection-spec.json");
const LIMIT: usize = 128_000;

pub(crate) fn handle(event: &str, data: Value, args: &[String]) -> Result<Value> {
    ensure!(matches!(event, "analysis_prepare" | "analysis_result"), "unsupported reflection event");
    let (config, base) = configured(args)?;
    let config = config.as_object().context("reflection config must be an object")?;
    ensure!(config.keys().all(|key| matches!(key.as_str(), "enabled" | "prompt")), "unknown reflection setting");
    let enabled = config
        .get("enabled")
        .map(Value::as_bool)
        .unwrap_or(Some(true))
        .context("reflection.enabled must be boolean")?;
    ensure!(enabled, "reflection plugin is disabled");
    match event {
        "analysis_prepare" => {
            ensure!(text(&data, "name")? == "reflection", "unknown reflection analysis");
            ensure!(data["transcript"].is_object(), "reflection requires a projected transcript object");
            ensure!(
                data.as_object().is_some_and(|object| object
                    .keys()
                    .all(|key| matches!(key.as_str(), "name" | "transcript" | "model"))),
                "reflection accepts only name, transcript and model"
            );
            let prompt = if let Some(path) = config.get("prompt") {
                let path = base.join(path.as_str().context("reflection.prompt must be a path")?);
                String::from_utf8(state::read_bounded(&path, LIMIT)?)?
            } else {
                let spec: Value = serde_json::from_str(SPEC)?;
                text(&spec, "prompt")?.to_owned()
            };
            ensure!(
                !prompt.trim().is_empty() && prompt.len() <= LIMIT,
                "reflection prompt must contain 1..128000 bytes"
            );
            Ok(json!({"prompt":prompt}))
        }
        "analysis_result" => {
            let summary = text(&data, "summary")?;
            ensure!(
                !summary.trim().is_empty() && summary.len() <= LIMIT,
                "reflection summary must contain 1..128000 bytes"
            );
            let mut result = json!({"summary":summary});
            if let Some(state) = data.get("state") {
                result["state"] = state.clone();
            }
            Ok(result)
        }
        _ => bail!("unsupported reflection event"),
    }
}
