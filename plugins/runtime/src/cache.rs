//! Provider cache controls and accounting. Prefix reuse is a prediction, never a hit metric.
use super::{configured, context::digest, state};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, PartialEq)]
pub(crate) struct Usage {
    pub input: u64,
    pub read: u64,
    pub write: u64,
}
pub(crate) fn usage(value: &Value) -> Option<Usage> {
    let zero = json!(0);
    let result = if value.get("prompt_tokens").is_some() {
        let input = value["prompt_tokens"].as_u64()?;
        let read = value
            .get("prompt_cache_hit_tokens")
            .unwrap_or(&value["prompt_tokens_details"]["cached_tokens"])
            .as_u64()?;
        if let Some(miss) = value.get("prompt_cache_miss_tokens")
            && read.checked_add(miss.as_u64()?)? != input
        {
            return None;
        }
        if value.get("prompt_cache_hit_tokens").is_some()
            && let Some(cached) = value["prompt_tokens_details"].get("cached_tokens")
            && cached.as_u64()? != read
        {
            return None;
        }
        Usage {
            input,
            read,
            write: value["prompt_tokens_details"].get("cache_write_tokens").unwrap_or(&zero).as_u64()?,
        }
    } else if value.get("cache_read_input_tokens").is_some() {
        let read = value["cache_read_input_tokens"].as_u64()?;
        let write = value.get("cache_creation_input_tokens").unwrap_or(&zero).as_u64()?;
        Usage { input: value["input_tokens"].as_u64()?.checked_add(read)?.checked_add(write)?, read, write }
    } else {
        Usage {
            input: value["input_tokens"].as_u64()?,
            read: value["input_tokens_details"]["cached_tokens"].as_u64()?,
            write: value["input_tokens_details"].get("cache_write_tokens").unwrap_or(&zero).as_u64()?,
        }
    };
    (result.read.checked_add(result.write)? <= result.input).then_some(result)
}
fn empty() -> Value {
    json!({"version":1,"totals":{},"phases":{},"streams":{},"recent":[]})
}
fn path(root: &Path, config: &Value) -> Result<PathBuf> {
    match config.get("stats") {
        Some(value) => state::inside(root, Path::new(value.as_str().context("stats must be a relative path")?)),
        None => state::workspace_state(root, "metrics/cache-stats.json"),
    }
}
fn read(path: &Path) -> Result<Value> {
    let value =
        if path.try_exists()? { serde_json::from_slice(&state::read_bounded(path, 2_000_000)?)? } else { empty() };
    ensure!(
        value["version"] == 1
            && ["totals", "phases", "streams"].iter().all(|k| value[k].is_object())
            && value["recent"].is_array(),
        "invalid cache statistics"
    );
    Ok(value)
}
fn update<T>(root: &Path, config: &Value, action: impl FnOnce(&mut Value) -> Result<T>) -> Result<T> {
    let path = path(root, config)?;
    fs::create_dir_all(path.parent().context("missing stats directory")?)?;
    let _guard = state::Lock::acquire(&path.with_extension("lock"))?;
    let mut value = read(&path)?;
    let result = action(&mut value)?;
    ensure!(serde_json::to_vec_pretty(&value)?.len() <= 2_000_000, "cache statistics exceed 2 MB");
    state::atomic(&path, &value)?;
    Ok(result)
}
fn add(value: &mut Value, key: &str, number: u64) -> Result<()> {
    let old = value.get(key).map_or(Some(0), Value::as_u64).context("invalid statistics counter")?;
    value[key] = json!(old.checked_add(number).context("statistics overflow")?);
    Ok(())
}
fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}
fn target(config: &Value) -> Result<f64> {
    let value = config.get("target_hit_ratio").map_or(Some(0.99), Value::as_f64).context("invalid cache target")?;
    ensure!(value.is_finite() && value > 0.0 && value <= 1.0, "cache target must be in (0, 1]");
    Ok(value)
}
fn account(value: &mut Value, usage: Option<&Usage>) -> Result<()> {
    add(value, "observed_responses", 1)?;
    if let Some(usage) = usage {
        add(value, "measured_responses", 1)?;
        add(value, "input_tokens", usage.input)?;
        add(value, "cache_read_tokens", usage.read)?;
        add(value, "cache_write_tokens", usage.write)?;
        add(value, "uncached_input_tokens", usage.input - usage.read)?;
    } else {
        add(value, "unknown_usage_responses", 1)?;
    }
    Ok(())
}
fn view(mut counters: Value, target: f64) -> Value {
    let input = counters["input_tokens"].as_u64().unwrap_or(0);
    let ratio = (input > 0).then(|| counters["cache_read_tokens"].as_u64().unwrap_or(0) as f64 / input as f64);
    let missing = counters["prepared_requests"]
        .as_u64()
        .unwrap_or(0)
        .saturating_sub(counters["observed_responses"].as_u64().unwrap_or(0));
    let complete = missing == 0 && counters["unknown_usage_responses"].as_u64().unwrap_or(0) == 0;
    counters["observed_token_hit_ratio"] = json!(ratio);
    counters["target_hit_ratio"] = json!(target);
    counters["unobserved_prepared_requests"] = json!(missing);
    counters["measurement_complete"] = json!(complete);
    counters["target_met"] = json!(ratio.filter(|_| complete).map(|r| r >= target));
    counters
}
pub(crate) fn report(root: &Path, args: &[String]) -> Result<Value> {
    let (config, _) = configured(args)?;
    let target = target(&config)?;
    let mut data = read(&path(root, &config)?)?;
    data["totals"] = view(data["totals"].take(), target);
    for phase in data["phases"].as_object_mut().unwrap().values_mut() {
        *phase = view(phase.take(), target);
    }
    data.as_object_mut().unwrap().remove("streams");
    data["note"] = json!(
        "Ratios use returned input-token usage, not local prefix equality. All prepared requests are counted; missing/error responses are not assumed to hit. Phase append predicts a stable prefix but includes real cache misses. No live-provider 99% guarantee."
    );
    Ok(data)
}
pub(crate) fn handle(event: &str, mut data: Value, root: &Path, args: &[String]) -> Result<Value> {
    let (config, _) = configured(args)?;
    if config["enabled"] == false {
        return Ok(data);
    }
    target(&config)?;
    if event == "session_load" {
        let first = data["messages"]
            .as_array_mut()
            .and_then(|messages| messages.first_mut())
            .context("missing system message")?;
        if !first["_cache_session"].is_string() {
            first["_cache_session"] = json!(uuid::Uuid::new_v4().to_string());
        }
    } else if event == "before_request" {
        ensure!(data["request_meta"].is_null() || data["request_meta"].is_object(), "request_meta must be an object");
        ensure!(data["request_meta"]["cache"].is_null(), "duplicate cache before_request hook");
        let origin = url::Url::parse(super::text(&data, "url")?)?.origin().ascii_serialization();
        let prefix = digest(&json!({"model":data["body"]["model"],"system":data["body"]["system"],
            "instructions":data["body"]["instructions"],"tools":data["body"]["tools"],
            "history_system":data["history"][0]["content"],"origin":origin,"namespace":config["namespace"]}));
        match config["mode"].as_str().unwrap_or("observe") {
            "observe" => {}
            "openai" => {
                let body = data["body"].as_object_mut().context("missing request body")?;
                body.entry("prompt_cache_key").or_insert_with(|| json!(prefix));
                if let Some(retention) = config.get("retention") {
                    ensure!(
                        matches!(retention.as_str(), Some("in_memory" | "24h")),
                        "unsupported OpenAI cache retention"
                    );
                    body.entry("prompt_cache_retention").or_insert(retention.clone());
                }
            }
            "anthropic" => {
                let ttl = config["ttl"].as_str().unwrap_or("5m");
                ensure!(["5m", "1h"].contains(&ttl), "Anthropic TTL must be 5m or 1h");
                data["body"]
                    .as_object_mut()
                    .context("missing body")?
                    .entry("cache_control")
                    .or_insert_with(|| json!({"type":"ephemeral","ttl":ttl}));
            }
            _ => anyhow::bail!("cache mode must be observe, openai or anthropic"),
        }
        // Observe the transformed request, never rewrite history text to improve a local score.
        let mut settings = data["body"].clone();
        let object = settings.as_object_mut().context("missing request body")?;
        object.remove("messages");
        object.remove("input");
        let prefix = digest(&json!({"url":data["url"],"settings":settings,"namespace":config["namespace"]}));
        let request_hash = digest(&data["body"]);
        let sequence = data["body"].get("input").unwrap_or(&data["body"]["messages"]).clone();
        let sequence = sequence.as_array().context("cache tracking requires a message/input array")?;
        let compact = data["compact"] == true;
        let session = data["history"][0]["_cache_session"].as_str().unwrap_or("").to_owned();
        let stream = digest(&json!([session, origin, data["body"]["model"], config["namespace"]]));
        let (phase, reason) = update(root, &config, |stats| {
            let previous = &stats["streams"][&stream];
            let length = previous["length"].as_u64().unwrap_or(u64::MAX);
            let phase = if compact {
                "compact"
            } else if session.is_empty() || previous.is_null() {
                "new"
            } else if previous["prefix"] == prefix
                && length <= sequence.len() as u64
                && previous["hash"] == digest(&json!(sequence[..length as usize]))
            {
                "append"
            } else {
                "reset"
            };
            let reason = match phase {
                "reset" if previous["prefix"] != prefix => "request_settings_changed",
                "reset" => "history_changed",
                "append" => "previous_message_prefix_preserved",
                "compact" => "compaction_request",
                _ => "no_previous_request",
            };
            if stats["phases"].get(phase).is_none() {
                stats["phases"][phase] = json!({});
            }
            add(&mut stats["totals"], "prepared_requests", 1)?;
            add(&mut stats["phases"][phase], "prepared_requests", 1)?;
            Ok((phase, reason))
        })?;
        if data["request_meta"].is_null() {
            data["request_meta"] = json!({});
        }
        data["request_meta"]["cache"] = json!({"id":uuid::Uuid::new_v4().to_string(),"phase":phase,
            "started_ms":now(),"model":data["body"]["model"].as_str().unwrap_or("").chars().take(256).collect::<String>(),
            "stream":stream,"track":!compact && !session.is_empty(),"prefix":prefix,"length":sequence.len(),"hash":digest(&json!(sequence)),
            "reason":reason,"request_hash":request_hash});
    } else if event == "request_retry" {
        let meta = &data["request_meta"]["cache"];
        if !meta.is_null() {
            let phase = super::text(meta, "phase")?;
            update(root, &config, |stats| {
                ensure!(stats["phases"][phase].is_object(), "retry without preparation");
                add(&mut stats["totals"], "prepared_requests", 1)?;
                add(&mut stats["phases"][phase], "prepared_requests", 1)
            })?;
        }
    } else if event == "after_response" {
        let meta = &data["request_meta"]["cache"];
        if meta.is_null() {
            return Ok(data);
        }
        let id = super::text(meta, "id")?;
        let phase = super::text(meta, "phase")?;
        let measured = usage(&data["response"]["usage"]);
        update(root, &config, |stats| {
            if stats["recent"].as_array().unwrap().iter().any(|item| item["id"] == id) {
                return Ok(());
            }
            ensure!(stats["phases"][phase].is_object(), "cache response without matching preparation");
            account(&mut stats["totals"], measured.as_ref())?;
            account(&mut stats["phases"][phase], measured.as_ref())?;
            if meta["track"] == true {
                stats["streams"][super::text(meta, "stream")?] =
                    json!({"prefix":meta["prefix"],"hash":meta["hash"],"length":meta["length"],"updated":now()});
                let streams = stats["streams"].as_object_mut().unwrap();
                if streams.len() > 256 {
                    let oldest =
                        streams.iter().min_by_key(|(_, v)| v["updated"].as_u64().unwrap_or(0)).unwrap().0.clone();
                    streams.remove(&oldest);
                }
            }
            let record = json!({"id":id,"model":meta["model"],"phase":phase,
                "prefix_diagnostic":meta["reason"],"request_hash":meta["request_hash"],
                "pipeline_elapsed_ms":now().saturating_sub(meta["started_ms"].as_u64().unwrap_or(0)),
                "input_tokens":measured.as_ref().map(|u|u.input),"cache_read_tokens":measured.as_ref().map(|u|u.read),
                "cache_write_tokens":measured.as_ref().map(|u|u.write),
                "uncached_input_tokens":measured.as_ref().map(|u|u.input-u.read),"response_status":data["response"]["status"]});
            let recent = stats["recent"].as_array_mut().unwrap();
            recent.push(record);
            if recent.len() > 200 {
                recent.remove(0);
            }
            Ok(())
        })?;
    }
    Ok(data)
}
