use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};

pub fn user_content(text: &str, images: &[String]) -> Result<Value> {
    ensure!(!text.trim().is_empty() || !images.is_empty(), "message cannot be empty");
    ensure!(text.len() <= 64 * 1024, "message text exceeds 64 KiB");
    ensure!(images.len() <= 8, "at most 8 images per message");
    let mut content = Vec::new();
    if !text.is_empty() {
        content.push(json!({"type":"text","text":text}));
    }
    let mut total = 0;
    for image in images {
        ensure!(image.len() <= 12 * 1024 * 1024, "image exceeds 8 MiB");
        let (header, encoded) = image.split_once(";base64,").context("image must be a base64 data URL")?;
        let bytes = STANDARD.decode(encoded).context("invalid image base64")?;
        ensure!(bytes.len() <= 8 * 1024 * 1024, "image exceeds 8 MiB");
        total += bytes.len();
        ensure!(total <= 16 * 1024 * 1024, "images exceed 16 MiB total");
        ensure!(
            header == format!("data:{}", crate::workspace::text::image_mime(&bytes)?),
            "image MIME does not match bytes"
        );
        content.push(json!({"type":"image_url","image_url":{"url":image}}));
    }
    Ok(if images.is_empty() { json!(text) } else { json!(content) })
}
