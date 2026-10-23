//! Replaceable compaction policy; HTTP, tool-batch boundaries and commit validation stay in the core.
use super::{budget, configured, state, text};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};

pub(crate) fn handle(event: &str, mut data: Value, workspace: &str, args: &[String]) -> Result<Value> {
    if !matches!(event, "compact_plan" | "before_compact") {
        return Ok(data);
    }
    let (config, base) = configured(args)?;
    ensure!(config.is_object(), "compact config must be an object");
    let enabled =
        config.get("enabled").map(Value::as_bool).unwrap_or(Some(true)).context("compact.enabled must be boolean")?;
    if event == "compact_plan" {
        data["cut"] = Value::Null;
        if !enabled || data["compact"] != true {
            return Ok(data);
        }
        let keep = config
            .get("keep_recent")
            .map(Value::as_u64)
            .unwrap_or(Some(4))
            .filter(|n| (1..=10000).contains(n))
            .context("compact.keep_recent must be 1..10000")? as usize;
        let history = data["history"].as_array().context("missing compaction history")?;
        let cut = (2..history.len().saturating_sub(keep.saturating_sub(1)))
            .rev()
            .find(|&i| matches!(history[i]["role"].as_str(), Some("user" | "assistant")));
        data["cut"] = json!(cut);
        return Ok(data);
    }
    ensure!(enabled, "compaction plugin is disabled");
    data["policy"] = budget::policy(&data)?;
    if let Some(images) = config.get("images") {
        ensure!(images.is_string(), "compact.images must be a string");
        data["policy"]["images"] = images.clone();
    }
    if let Some(path) = config.get("prompt") {
        let path = base.join(path.as_str().context("compact.prompt must be a path")?);
        data["prompt"] = json!(String::from_utf8(state::read_bounded(&path, 128_000)?)?);
    } else if data["prompt"].is_null() {
        data["prompt"] = json!(include_str!("../../prompts/compact.txt"));
    }
    let prompt = text(&data, "prompt")?;
    ensure!(!prompt.trim().is_empty() && prompt.chars().count() <= 32_000, "prompt must contain 1..32000 characters");
    data["prompt"] = json!(prompt.replace("{{workspace}}", workspace));
    prepare(data)
}
fn clean(value: Value, images: &mut Vec<Value>, mode: &str) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(|item| clean(item, images, mode)).collect()),
        Value::Object(mut object) => {
            if object.get("type") == Some(&json!("image_url")) {
                let image = Value::Object(object);
                if !images.iter().any(|old| old["image_url"]["url"] == image["image_url"]["url"]) {
                    images.push(image);
                }
                return json!({"type":"text","text":format!("[old image handled separately: {mode}]")});
            }
            object.retain(|key, _| !key.starts_with('_'));
            Value::Object(object.into_iter().map(|(key, item)| (key, clean(item, images, mode))).collect())
        }
        Value::String(text) if text.starts_with("data:image/") => {
            json!("[unstructured image data omitted; do not infer its contents]")
        }
        other => other,
    }
}
pub(crate) fn prepare(mut data: Value) -> Result<Value> {
    let policy = budget::policy(&data)?;
    let mode = policy.get("images").map(Value::as_str).unwrap_or(Some("keep")).context("images must be a string")?;
    ensure!(["keep", "omit", "describe"].contains(&mode), "images must be keep, omit or describe");
    let mut images = Vec::new();
    data["messages"] = clean(data["messages"].take(), &mut images, mode);
    data["retained_images"] = if mode == "keep" { json!(images) } else { json!([]) };
    data["summary_images"] = if mode == "describe" { json!(images) } else { json!([]) };
    data["prompt"] = json!(format!(
        "{}\nImage policy: {mode}. Never treat base64 as text or infer unseen image details. If images are attached, summarize relevant visual facts; otherwise note missing visual details and use the original image when needed.",
        text(&data, "prompt")?
    ));
    Ok(data)
}
