//! Replaceable wire adapters. Credentials and HTTP execution belong to the host.
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
#[path = "provider_wire.rs"]
mod wire;
#[derive(Clone, Copy, PartialEq)]
enum Protocol {
    Openai,
    Anthropic,
}
fn field<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
    super::text(value, name)
}

pub(crate) fn handle(event: &str, data: Value) -> Result<Value> {
    ensure!(data["version"] == 1, "unsupported provider contract");
    let kind = field(&data, "protocol")?;
    let protocol = match kind {
        "openai" | "deepseek" | "responses" => Protocol::Openai,
        "anthropic" => Protocol::Anthropic,
        _ => bail!("unknown provider adapter: {kind}"),
    };
    match event {
        "provider_prepare" => {
            for key in data["extra_body"].as_object().context("missing extra_body")?.keys() {
                ensure!(
                    !"model messages system tools tool_choice max_tokens stream input instructions \
                    max_output_tokens max_completion_tokens store previous_response_id conversation background"
                        .split_whitespace()
                        .any(|reserved| reserved == key),
                    "extra_body cannot override reserved field: {key}"
                );
            }
            let base = field(&data, "base_url")?;
            let history = data["history"].as_array().context("missing history")?;
            let max_tokens = data["max_tokens"].as_u64().context("missing max_tokens")?;
            let mut definitions: Vec<Value> =
                data["tools"].as_array().context("missing tools")?.iter().map(|t| t["function"].clone()).collect();
            definitions.extend(data["plugin_tools"].as_array().context("missing plugin tools")?.iter().cloned());
            let mut body = wire::request_body(protocol, field(&data, "model")?, history, max_tokens, &definitions)?;
            if protocol == Protocol::Openai && (data["use_max_tokens"] == true || kind == "deepseek") {
                body.as_object_mut().unwrap().remove("max_completion_tokens");
                body["max_tokens"] = json!(max_tokens);
            }
            if data["compact"] == true {
                body.as_object_mut().unwrap().remove("tools");
                body.as_object_mut().unwrap().remove("tool_choice");
            }
            body.as_object_mut().unwrap().extend(data["extra_body"].as_object().context("missing extra_body")?.clone());
            let mut prepared = json!({"version":1,"url":wire::endpoint(base,protocol),"body":body,
                "headers":{},"auth":{"header":"authorization","prefix":"Bearer "}});
            if protocol == Protocol::Anthropic {
                prepared["headers"] = json!({"anthropic-version":"2023-06-01"});
                prepared["auth"] = json!({"header":"x-api-key","prefix":""});
            } else if kind == "responses" {
                let converted = super::responses::handle(
                    "before_request",
                    json!({"url":prepared["url"],
                    "body":prepared["body"],"history":history,"compact":data["compact"]}),
                )?;
                prepared["url"] = converted["url"].clone();
                prepared["body"] = converted["body"].clone();
            }
            Ok(prepared)
        }
        "provider_parse" => {
            let response = if kind == "responses" {
                super::responses::handle("after_response", json!({"response":data["response"]}))?["response"].take()
            } else {
                wire::normalize_response(protocol, data["response"].clone())?
            };
            Ok(json!({"version":1,"response":response}))
        }
        _ => bail!("unknown provider event: {event}"),
    }
}
