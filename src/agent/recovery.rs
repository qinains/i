use crate::{
    agent::history::{checked_history, tool_calls},
    workspace::{field, text::image_mime},
};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Reconstruct a native or browser batch without reauthorizing or executing any call.
pub fn recover(history: &[Value], journal: Option<&Value>) -> Result<Vec<Value>> {
    let mut history = checked_history(json!(history))?;
    let Some(journal) = journal else {
        return Ok(history);
    };
    ensure!(journal["assistant"]["role"] == "assistant", "invalid recovery journal assistant");
    let calls = tool_calls(&journal["assistant"])?;
    ensure!(!calls.is_empty(), "invalid recovery journal batch");
    let mut saved = BTreeMap::new();
    for item in journal["calls"].as_array().context("invalid recovery journal calls")? {
        let id = field(item, "id")?;
        ensure!(
            calls.iter().any(|call| call["id"] == id) && saved.insert(id, item).is_none(),
            "unknown/duplicate recovery journal call"
        );
        ensure!(
            item.get("status").is_none() || matches!(item["status"].as_str(), Some("pending" | "started" | "finished")),
            "invalid recovery journal status"
        );
    }
    history.push(journal["assistant"].clone());
    let mut images = Vec::new();
    for call in calls {
        let item = saved.get(field(&call, "id")?).copied();
        let output = match item.and_then(|item| item["status"].as_str()) {
            Some("finished") => {
                let result = &item.context("missing recovery result")?["result"];
                let output = field(result, "output")?;
                if let Some(saved_images) = result.get("images") {
                    for image in saved_images.as_array().context("invalid recovery images")? {
                        ensure!(
                            images.len() / 2 < 8 && image["type"] == "image_url",
                            "invalid/excessive recovery images"
                        );
                        let url = field(&image["image_url"], "url")?;
                        ensure!(url.len() <= 12 * 1024 * 1024, "recovery image exceeds encoded size limit");
                        let (header, encoded) =
                            url.split_once(";base64,").context("recovery image must be a data URL")?;
                        let bytes = STANDARD.decode(encoded).context("invalid recovery image base64")?;
                        ensure!(
                            bytes.len() <= 8 * 1024 * 1024 && header == format!("data:{}", image_mime(&bytes)?),
                            "invalid recovery image bytes or MIME"
                        );
                        images.push(json!({"type":"text","text":format!("Image from tool {}",call["id"])}));
                        images.push(image.clone());
                    }
                }
                output
            }
            Some("started") => {
                "INTERRUPTED: execution outcome unknown. Inspect workspace files; do not automatically repeat this operation."
            }
            _ => "INTERRUPTED: not executed. No previous approval is valid.",
        };
        history.push(json!({"role":"tool","tool_call_id":call["id"],"content":output}));
    }
    if !images.is_empty() {
        history.push(json!({"role":"user","content":images}));
    }
    checked_history(json!(history))
}
