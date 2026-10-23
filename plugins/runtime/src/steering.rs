use super::state;
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{collections::HashSet, path::Path};

fn update<T>(path: &Path, change: impl FnOnce(&mut Vec<Value>) -> Result<T>) -> Result<T> {
    let _guard = state::Lock::acquire(&path.with_extension("lock"))?;
    let mut state = if path.try_exists()? {
        serde_json::from_slice(&state::read_bounded(path, 16 * 1024 * 1024)?)?
    } else {
        json!({"items":[]})
    };
    let result = change(state["items"].as_array_mut().context("invalid inbox")?)?;
    ensure!(serde_json::to_vec(&state)?.len() <= 16 * 1024 * 1024, "steering inbox too large");
    state::atomic(path, &state)?;
    Ok(result)
}
pub(crate) fn enqueue(path: &Path, text: &str) -> Result<String> {
    ensure!(!text.trim().is_empty(), "empty steering input");
    let id = uuid::Uuid::new_v4().simple().to_string();
    update(path, |items| {
        items.push(json!({"id":id,"text":text}));
        Ok(())
    })?;
    Ok(id)
}
pub(crate) fn pending(path: &Path) -> Result<Vec<Value>> {
    update(path, |items| Ok(items.clone()))
}
pub(crate) fn handle(event: &str, mut data: Value, path: &Path) -> Result<Value> {
    if !["before_turn", "after_save"].contains(&event) {
        return Ok(data);
    }
    let messages = data["messages"].as_array_mut().context("missing messages")?;
    let seen: HashSet<String> = messages.iter().filter_map(|m| m["_steering_id"].as_str().map(str::to_owned)).collect();
    if event == "before_turn" {
        for item in pending(path)? {
            if !seen.contains(super::text(&item, "id")?) {
                messages.push(json!({"role":"user","content":item["text"],"_steering_id":item["id"]}));
            }
        }
    } else {
        update(path, |items| {
            items.retain(|item| !item["id"].as_str().is_some_and(|id| seen.contains(id)));
            Ok(())
        })?;
    }
    Ok(data)
}
