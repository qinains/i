use super::{context, state, text};
use anyhow::{Result, bail, ensure};
use serde_json::{Value, json};
use std::{fs, path::Path};
const LIMIT: usize = 16_000;
fn read(path: &Path) -> Result<Value> {
    if !path.try_exists()? {
        return Ok(json!({}));
    }
    let value: Value = serde_json::from_slice(&state::read_bounded(path, LIMIT)?)?;
    ensure!(
        value.as_object().is_some_and(|map| map.values().all(Value::is_string)),
        "memory must map strings to strings"
    );
    Ok(value)
}
pub(crate) fn handle(event: &str, mut data: Value, workspace: &Path) -> Result<Value> {
    let path = state::workspace_state(workspace, "memory.json")?;
    if ["before_turn", "before_compact_commit"].contains(&event) {
        context::update(&mut data, "memory", read(&path)?)?;
    } else if event == "tool" {
        let args = &data["arguments"];
        match text(&data, "name")? {
            "memory_get" => {
                let memory = read(&path)?;
                let result = if args.get("key").is_some() { &memory[text(args, "key")?] } else { &memory };
                return Ok(json!({"output":result.to_string()}));
            }
            name @ ("memory_set" | "memory_delete") => {
                let key = text(args, "key")?;
                ensure!(
                    !key.is_empty()
                        && key.len() <= 64
                        && key.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b)),
                    "invalid memory key"
                );
                fs::create_dir_all(path.parent().unwrap())?;
                let _guard = state::Lock::acquire(&path.with_extension("lock"))?;
                let mut memory = read(&path)?;
                if name == "memory_set" {
                    let value = text(args, "value")?;
                    ensure!(
                        !value.trim().is_empty() && value.chars().count() <= 2000,
                        "memory value must be 1..2000 characters"
                    );
                    memory[key] = json!(value);
                } else {
                    memory.as_object_mut().unwrap().remove(key);
                }
                ensure!(
                    serde_json::to_vec_pretty(&memory)?.len() <= LIMIT,
                    "memory exceeds 16000 bytes; delete obsolete entries first"
                );
                state::atomic(&path, &memory)?;
                return Ok(
                    json!({"output":"Memory updated. Reference changes will be appended at the next safe turn boundary."}),
                );
            }
            _ => bail!("unknown memory tool"),
        }
    }
    Ok(data)
}
