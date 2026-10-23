//! Optional replacement demo: local extractive notes, not a semantic/LLM summary.
//! Old image attachments are deliberately omitted; the host retains its backup.
use serde_json::{Value, json};
use std::error::Error;

fn prefix(text: &str, limit: usize) -> &str {
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}
pub(super) fn handle(event: &Value) -> Result<Value, Box<dyn Error>> {
    let mut data = event["data"].clone();
    if !data.is_object() {
        return Err("data must be an object".into());
    }
    match event["event"].as_str() {
        Some("compact_plan") => {
            data["cut"] = Value::Null;
            if data["compact"] == true {
                let history = data["history"].as_array().ok_or("missing history")?;
                data["cut"] = json!(
                    (2..history.len().saturating_sub(3))
                        .rev()
                        .find(|&i| matches!(history[i]["role"].as_str(), Some("user" | "assistant")))
                );
                data["state"] = json!({"strategy":"local-extractive-demo"});
            }
        }
        Some("before_compact") => {
            let messages = data["messages"].as_array().ok_or("missing messages")?;
            let mut summary = String::from(
                "Extractive notes, truncated; NOT a semantic summary. Older images omitted; consult the precompact backup.\n",
            );
            for message in messages {
                let role = message["role"].as_str().unwrap_or("unknown");
                let content = if let Some(text) = message["content"].as_str() {
                    text.to_owned()
                } else if let Some(parts) = message["content"].as_array() {
                    parts
                        .iter()
                        .filter(|p| p["type"] == "text")
                        .filter_map(|p| p["text"].as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                } else {
                    "[non-text message omitted]".into()
                };
                let content = if content.contains("data:image/") { "[image data omitted]" } else { &content };
                let line = format!("{role}: {}\n", prefix(content, 512));
                summary.push_str(prefix(&line, 2048usize.saturating_sub(summary.len())));
                if summary.len() >= 2044 {
                    break;
                }
            }
            data["summary"] = json!(summary); // Presence skips the host's model-summary request.
            data["retained_images"] = json!([]);
        }
        _ => {}
    }
    Ok(data)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extractive_output_is_bounded_utf8_and_omits_image_and_adapter_data() {
        let data = handle(&json!({"event":"before_compact","data":{"messages":[
            {"role":"user","content":[{"type":"text","text":"中文".repeat(3000)},
                {"type":"image_url","image_url":{"url":"data:image/png;base64,PRIVATE"}}],"_usage":"PRIVATE"},
            {"role":"tool","content":"x".repeat(3000)}]}}))
        .unwrap();
        let summary = data["summary"].as_str().unwrap();
        assert!(summary.len() <= 2048 && summary.contains("中文") && !summary.contains("PRIVATE"));
        assert_eq!(data["retained_images"], json!([]));
    }
}
