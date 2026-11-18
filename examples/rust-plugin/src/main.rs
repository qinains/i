use serde_json::{Value, json};
use std::{
    error::Error,
    io::{self, BufRead, Read, Write},
};

mod compact;
// Persistent NDJSON worker: one request ID and one response per line.
fn handle(event: &Value) -> Result<Value, Box<dyn Error>> {
    let mut data = event["data"].clone();
    match event["event"].as_str() {
        Some("system_prompt") => {
            let prompt = data["prompt"].as_str().ok_or("missing prompt")?;
            data["prompt"] = json!(format!("{prompt}\nPrefer focused changes with explicit test results."));
        }
        Some("tool") => {
            if data["name"] != "rust_word_count" {
                return Err("unknown tool".into());
            }
            let text = data["arguments"]["text"].as_str().ok_or("text must be a string")?;
            data = json!({"output":json!({"words":text.split_whitespace().count(),"characters":text.chars().count()}).to_string()});
        }
        _ => {}
    }
    Ok(data)
}
fn main() -> Result<(), Box<dyn Error>> {
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    let compactor = std::env::args().any(|arg| arg == "--compact");
    loop {
        let mut bytes = Vec::new();
        input.by_ref().take(48 * 1024 * 1024 + 1).read_until(b'\n', &mut bytes)?;
        if bytes.is_empty() {
            break;
        }
        if bytes.len() > 48 * 1024 * 1024 || bytes.last() != Some(&b'\n') {
            return Err("invalid frame".into());
        }
        let event: Value = serde_json::from_slice(&bytes)?;
        let id = event["id"].as_u64().ok_or("missing id")?;
        let result = match if compactor { compact::handle(&event) } else { handle(&event) } {
            Ok(data) => json!({"id":id,"data":data}),
            Err(error) => json!({"id":id,"error":error.to_string()}),
        };
        serde_json::to_writer(&mut output, &result)?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
    Ok(())
}
