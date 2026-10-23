//! Low-cost API connectivity check policy; never permits model-requested tools.
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
pub(crate) fn handle(event: &str, mut data: Value, args: &[String]) -> Result<Value> {
    if event == "before_request" {
        let cap = super::option(args, "--output-cap").map(|value| value.parse::<u64>()).transpose()?.unwrap_or(32);
        ensure!((1..=1024).contains(&cap), "smoke output cap must be 1..1024");
        let body = data["body"].as_object_mut().context("missing body")?;
        body.remove("tools");
        body.remove("tool_choice");
        let mut limited = false;
        for key in ["max_tokens", "max_completion_tokens", "max_output_tokens"] {
            if let Some(value) = body.get_mut(key) {
                *value = json!(value.as_u64().context("invalid output budget")?.min(cap));
                limited = true;
            }
        }
        ensure!(limited, "smoke request requires an output limit");
    } else if event == "after_response" {
        let response = &data["response"];
        ensure!(
            response["choices"][0]["message"]["tool_calls"].as_array().is_none_or(|calls| calls.is_empty()),
            "smoke check forbids tools"
        );
        for key in ["content", "output"] {
            ensure!(
                response[key].as_array().is_none_or(|items| items
                    .iter()
                    .all(|item| !matches!(item["type"].as_str(), Some("tool_use" | "function_call")))),
                "smoke check forbids tools"
            );
        }
    }
    Ok(data)
}
