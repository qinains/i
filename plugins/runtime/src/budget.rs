use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub(crate) fn fingerprint(history: &Value) -> String {
    // Only message-level local metadata is excluded. Provider replay state changes
    // the wire input, so it must remain part of the usage anchor.
    let mut wire = history.clone();
    if let Some(messages) = wire.as_array_mut() {
        for message in messages {
            if let Some(object) = message.as_object_mut() {
                object.retain(|key, _| {
                    !key.starts_with('_') || ["_responses_output", "_anthropic_content"].contains(&key.as_str())
                });
            }
        }
    }
    format!("{:x}", Sha256::digest(serde_json::to_vec(&wire).unwrap()))
}
pub(crate) fn policy(data: &Value) -> Result<Value> {
    let mut result = data["policy"].as_object().cloned().unwrap_or_default();
    if let Some(models) = result.remove("models") {
        let model = super::text(data, "model")?;
        let selected = models[model].as_object().with_context(|| format!("no budget metadata for model {model}"))?;
        result.extend(selected.clone());
    }
    Ok(Value::Object(result))
}
fn positive(value: &Value, name: &str) -> Result<u64> {
    value.as_u64().filter(|n| *n > 0).with_context(|| format!("{name} must be a positive integer"))
}
fn ratio(policy: &Value, key: &str, default: f64, zero: bool) -> Result<f64> {
    let number = match policy.get(key) {
        Some(v) => v.as_f64().context("invalid ratio")?,
        None => default,
    };
    ensure!(number.is_finite() && (0.0..1.0).contains(&number) && (zero || number > 0.0), "invalid {key}");
    Ok(number)
}
pub(crate) struct Counter {
    image: Option<u64>,
}
impl Counter {
    pub(crate) fn new(policy: &Value) -> Result<Self> {
        ensure!(
            policy.get("tokenizer").is_none_or(|value| value == "utf8_bytes"),
            "BPE tokenizers are not bundled; omit budget.tokenizer to use usage with conservative byte estimates"
        );
        Ok(Self { image: policy["image_tokens"].as_u64().filter(|n| *n > 0) })
    }
    fn text(&self, text: &str) -> u64 {
        text.len() as u64
    }
    pub(crate) fn count(&self, value: &Value) -> Result<u64> {
        Ok(match value {
            Value::String(text) => self.text(text),
            Value::Array(items) => {
                items.iter().try_fold(0, |sum, item| Ok::<_, anyhow::Error>(sum + self.count(item)? + 4))?
            }
            Value::Object(items) => {
                if matches!(value["type"].as_str(), Some("image_url" | "input_image" | "image")) {
                    self.image.context("image_tokens is required for images")?
                } else {
                    items.iter().filter(|(key, _)| !key.starts_with('_')).try_fold(0, |sum, (key, item)| {
                        Ok::<_, anyhow::Error>(sum + self.text(key) + self.count(item)? + 4)
                    })?
                }
            }
            _ => self.text(&value.to_string()),
        })
    }
}
pub(crate) fn usage_total(usage: &Value) -> Option<u64> {
    let (input, output, cache): (&str, &str, &[&str]) = if usage.get("prompt_tokens").is_some() {
        ("prompt_tokens", "completion_tokens", &[])
    } else if usage.get("input_tokens").is_some() {
        ("input_tokens", "output_tokens", &["cache_read_input_tokens", "cache_creation_input_tokens"])
    } else {
        return None;
    };
    // Missing or malformed usage is unknown, not zero. Cache subcounts are optional.
    let total = usage[input].as_u64()?.checked_add(usage[output].as_u64()?)?;
    cache.iter().try_fold(total, |sum, field| sum.checked_add(usage.get(field).map_or(Some(0), Value::as_u64)?))
}
pub(crate) fn budget(mut data: Value) -> Result<Value> {
    let policy = policy(&data)?;
    if policy["enabled"] == false {
        data.as_object_mut().unwrap().extend(
            json!({"estimated_input_tokens":0,"compact":false,"fits":true,"source":"disabled"})
                .as_object()
                .unwrap()
                .clone(),
        );
        return Ok(data);
    }
    let window = positive(&policy["context_window"], "context_window")?;
    ensure!(window <= 1_000_000_000_000, "context_window is implausibly large");
    let output = positive(&data["max_tokens"], "max_tokens")?;
    let safety = (window as f64 * ratio(&policy, "safety_ratio", 0.02, true)?).ceil() as u64;
    ensure!(output < window.saturating_sub(safety), "output reservation + safety margin exhaust the context window");
    let trigger =
        ((window as f64 * ratio(&policy, "compact_ratio", 0.8, false)?).floor() as u64).min(window - output - safety);
    let counter = Counter::new(&policy)?;
    let empty = json!([]);
    let schemas = if data["compact"] == true {
        0
    } else {
        counter.count(data.get("tools").unwrap_or(&empty))?
            + counter.count(data.get("plugin_tools").unwrap_or(&empty))?
    };
    let history = data["history"].as_array().context("missing history")?;
    let content_estimate = counter.count(&data["history"])? + schemas;
    let mut estimate = content_estimate;
    let mut source = "estimate";
    let scope = data.get("provider_scope").map(super::context::digest);
    for index in (0..history.len()).rev() {
        let saved = &history[index]["_usage"];
        if let Some(total) = usage_total(&saved["tokens"])
            && saved["anchor"]["model"] == data["model"]
            && saved["anchor"].get("scope").and_then(Value::as_str) == scope.as_deref()
            && saved["anchor"]["history_hash"] == fingerprint(&json!(history[..index]))
        {
            estimate =
                total.checked_add(counter.count(&json!(history[index + 1..]))? + schemas).context("usage overflow")?;
            source = "usage+estimate";
            break;
        }
    }
    let pending = data["pending_tools"].as_u64().unwrap_or(1).max(1);
    let tokens = ((window as f64 * ratio(&policy, "tool_output_ratio", 0.02, false)?).floor() as u64)
        .min(trigger.saturating_sub(estimate) / pending);
    data.as_object_mut().unwrap().extend(json!({"estimated_input_tokens":estimate,"content_estimate_tokens":content_estimate,"context_window":window,
        "input_trigger":trigger,"reserved_output_tokens":output,"safety_tokens":safety,"source":source,
        "compact":estimate>=trigger,"fits":estimate<window-output-safety,"tool_output_tokens":tokens,"tool_output_bytes":tokens}).as_object().unwrap().clone());
    Ok(data)
}
pub(crate) fn handle(event: &str, mut data: Value) -> Result<Value> {
    match event {
        "context_budget" => budget(data),
        "before_request" => {
            data["budget_anchor"] = json!({"history_hash":fingerprint(&data["history"]),"model":data["body"]["model"],
                "scope":data.get("provider_scope").map(super::context::digest)});
            Ok(data)
        }
        _ => Ok(data),
    }
}
