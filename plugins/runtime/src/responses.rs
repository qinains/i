use super::text;
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};

pub(crate) fn input(history: &Value) -> Result<Value> {
    let mut items = Vec::new();
    for message in history.as_array().context("missing history")? {
        if let Some(output) = message.get("_responses_output") {
            items.extend(output.as_array().context("invalid replay state")?.clone());
            continue;
        }
        let role = text(message, "role")?;
        if role == "tool" {
            items.push(
                json!({"type":"function_call_output","call_id":message["tool_call_id"],"output":message["content"]}),
            );
            continue;
        }
        let mut content = message["content"].clone();
        if let Some(parts) = content.as_array() {
            content = Value::Array(
                parts
                    .iter()
                    .map(|part| {
                        Ok(match text(part, "type")? {
                            "text" => json!({"type":"input_text","text":part["text"]}),
                            "file" => json!({"type":"input_image","file_id":text(part,"file_id")?}),
                            "image_url" => {
                                let mut result = json!({"type":"input_image","image_url":part["image_url"]["url"]});
                                if let Some(detail) = part["image_url"].get("detail") {
                                    result["detail"] = detail.clone();
                                }
                                result
                            }
                            _ => bail!("unsupported input content part"),
                        })
                    })
                    .collect::<Result<_>>()?,
            );
        }
        if !content.is_null() && content != "" && content != json!([]) {
            items.push(json!({"role":role,"content":content}));
        }
        if let Some(calls) = message["tool_calls"].as_array() {
            for call in calls {
                items.push(json!({"type":"function_call","call_id":call["id"],"name":call["function"]["name"],"arguments":call["function"]["arguments"]}));
            }
        }
    }
    Ok(json!(items))
}
pub(crate) fn normalize(response: &Value) -> Result<Value> {
    ensure!(
        response["status"] == "completed",
        "Responses not completed: {} {} {}",
        response["status"],
        response["incomplete_details"],
        response["error"]
    );
    let output = response["output"].as_array().context("missing output")?;
    let (mut texts, mut calls) = (Vec::new(), Vec::new());
    for item in output {
        match text(item, "type")? {
            "function_call" => {
                ensure!(item["arguments"].is_string(), "function arguments must be a JSON string");
                calls.push(json!({"id":item["call_id"],"type":"function","function":{"name":item["name"],"arguments":item["arguments"]}}));
            }
            "message" => {
                ensure!(item["role"] == "assistant", "expected assistant output");
                for part in item["content"].as_array().context("missing content")? {
                    texts.push(match text(part, "type")? {
                        "output_text" => text(part, "text")?,
                        "refusal" => text(part, "refusal")?,
                        _ => bail!("unsupported output content"),
                    });
                }
            }
            "reasoning" => {}
            kind => bail!("unsupported output item: {kind}"),
        }
    }
    Ok(json!({"usage":response["usage"],"choices":[{"finish_reason":if calls.is_empty(){"stop"}else{"tool_calls"},
        "message":{"role":"assistant","content":texts.join("\n"),"tool_calls":calls,"_responses_output":output}}]}))
}
pub(crate) fn handle(event: &str, mut data: Value) -> Result<Value> {
    if event == "before_request" {
        ensure!(
            data["body"].get("input").is_none(),
            "Responses already prepared; remove the legacy responses hook when selecting the responses provider"
        );
        data["url"] = json!(format!(
            "{}/responses",
            text(&data, "url")?
                .strip_suffix("/chat/completions")
                .context("Responses adapter requires protocol=openai")?
        ));
        let body = &data["body"];
        let mut result = body.as_object().context("missing body")?.clone();
        for key in ["messages", "tools", "tool_choice", "max_completion_tokens", "max_tokens"] {
            result.remove(key);
        }
        result.insert("input".into(), input(&data["history"])?);
        result.insert("store".into(), json!(false));
        let include = result.entry("include").or_insert_with(|| json!([]));
        let include = include.as_array_mut().context("Responses include must be an array")?;
        ensure!(include.iter().all(Value::is_string), "Responses include entries must be strings");
        if !include.iter().any(|item| item == "reasoning.encrypted_content") {
            include.push(json!("reasoning.encrypted_content"));
        }
        result.insert(
            "max_output_tokens".into(),
            body.get("max_completion_tokens").unwrap_or(&body["max_tokens"]).clone(),
        );
        if data["compact"] != true
            && let Some(tools) = body["tools"].as_array().filter(|t| !t.is_empty())
        {
            let tools = tools
                .iter()
                .map(|tool| {
                    let mut function = tool["function"].clone();
                    function["type"] = json!("function");
                    function["strict"] = json!(false);
                    function
                })
                .collect::<Vec<_>>();
            result.insert("tools".into(), json!(tools));
            result.insert("tool_choice".into(), json!("auto"));
        }
        data["body"] = Value::Object(result);
    } else if event == "after_response" {
        data["response"] = normalize(&data["response"])?;
    }
    Ok(data)
}
