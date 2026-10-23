use super::{configured, option, state};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::path::PathBuf;

pub(crate) fn handle(event: &str, mut data: Value, workspace: &str, args: &[String]) -> Result<Value> {
    let (flag, default) = match event {
        "system_prompt" => ("--system", include_str!("../../prompts/system.txt")),
        "before_compact" => ("--compact", include_str!("../../prompts/compact.txt")),
        _ => return Ok(data),
    };
    let (config, base) = configured(args)?;
    let path = option(args, flag)
        .map(PathBuf::from)
        .or_else(|| config[flag.trim_start_matches('-')].as_str().map(|path| base.join(path)));
    let content = match path {
        Some(path) => String::from_utf8(state::read_bounded(&path, 128_000)?)?,
        None => default.into(),
    };
    ensure!(!content.trim().is_empty() && content.chars().count() <= 32_000, "prompt must contain 1..32000 characters");
    data["prompt"] = json!(content.replace("{{workspace}}", workspace));
    Ok(data)
}
