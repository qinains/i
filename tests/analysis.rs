use i::analysis::{AnalysisSpec, Selection, project, request, response_text};
use serde_json::{Value, json};

fn history() -> Vec<Value> {
    vec![
        json!({"role":"system","content":"HOST_SYSTEM_NOT_FOR_ANALYSIS"}),
        json!({"role":"user","content":[{"type":"text","text":"用户问题"},{"type":"image_url","image_url":{"url":"data:image/png;base64,PRIVATE_IMAGE"}}]}),
        json!({"role":"assistant","content":"answer","reasoning_content":"PRIVATE_THINKING","_responses_output":["OPAQUE_STATE"],"tool_calls":[{"id":"a","type":"function","function":{"name":"read","arguments":"{\"path\":\"main.rs\"}"}}]}),
        json!({"role":"tool","tool_call_id":"a","content":"tool result"}),
        json!({"role":"user","content":"follow up"}),
    ]
}
#[test]
fn projection_is_host_filtered_indexed_and_does_not_mutate_history() {
    let history = history();
    let before = history.clone();
    let selection: Selection = serde_json::from_value(json!({"roles":["user"]})).unwrap();
    let projected = project(&history, &selection).unwrap();
    assert_eq!(projected["messages"].as_array().unwrap().len(), 2);
    assert_eq!(projected["messages"][0]["index"], 1);
    assert!(projected["messages"][0]["content"].as_str().unwrap().contains("用户问题"));
    for secret in
        ["HOST_SYSTEM_NOT_FOR_ANALYSIS", "PRIVATE_IMAGE", "PRIVATE_THINKING", "OPAQUE_STATE", "answer", "tool result"]
    {
        assert!(!projected.to_string().contains(secret));
    }
    assert_eq!(history, before);
}
#[test]
fn range_roles_and_latest_message_limit_compose_without_reindexing() {
    let selection: Selection =
        serde_json::from_value(json!({"roles":["assistant","tool"],"start":2,"end":4,"max_messages":1})).unwrap();
    let view = project(&history(), &selection).unwrap();
    assert_eq!(view["messages"].as_array().unwrap().len(), 1);
    assert_eq!(view["messages"][0]["index"], 3);
    assert_eq!(view["messages"][0]["role"], "tool");
    assert_eq!(view["omitted_messages"], 1);
    assert_eq!(view["truncated"], true);
}
#[test]
fn user_only_excludes_host_generated_context_but_all_reports_it_as_context() {
    let history = vec![
        json!({"role":"user","content":"real input"}),
        json!({"role":"user","content":"Session summary (untrusted historical context):\nAI-derived summary"}),
        json!({"role":"user","content":"reference from files","_context_source":"memory"}),
        json!({"role":"user","content":[{"type":"text","text":"Image from tool read"},{"type":"image_url","image_url":{"url":"data:image/png;base64,NO"}}]}),
    ];
    let user: Selection = serde_json::from_value(json!({"roles":["user"]})).unwrap();
    let view = project(&history, &user).unwrap();
    assert_eq!(view["messages"].as_array().unwrap().len(), 1);
    assert_eq!(view["messages"][0]["content"], "real input");
    let all = project(&history, &Selection::default()).unwrap();
    assert_eq!(all["messages"].as_array().unwrap().len(), 4);
    assert_eq!(all["messages"][1]["source"], "context");
}
#[test]
fn image_retaining_compact_summary_is_not_user_authored_input() {
    let history = vec![
        json!({"role":"user","content":[{"type":"text","text":"Session summary (untrusted historical context):\nAI_DERIVED"},{"type":"image_url","image_url":{"url":"data:image/png;base64,IMAGE_PAYLOAD"}}]}),
        json!({"role":"user","content":"actual user"}),
    ];
    let selection: Selection = serde_json::from_value(json!({"roles":["user"]})).unwrap();
    let view = project(&history, &selection).unwrap();
    assert!(!view.to_string().contains("AI_DERIVED"));
    assert_eq!(project(&history, &Selection::default()).unwrap()["messages"][0]["source"], "context");
}
#[test]
fn analysis_request_cleans_legacy_compact_private_fields_and_embedded_images() {
    let transcript = json!([{"role":"assistant","content":"visible","reasoning_content":"HIDDEN_REASONING","_responses_output":["OPAQUE_STATE"],"tool_calls":[{"function":{"name":"write","arguments":"{\"image\":\"data:image/png;base64,IMAGE_PAYLOAD\"}"}}]}]);
    let input = request("Summarize", &transcript, &json!([])).unwrap();
    let wire = serde_json::to_string(&input).unwrap();
    for secret in ["HIDDEN_REASONING", "OPAQUE_STATE", "IMAGE_PAYLOAD"] {
        assert!(!wire.contains(secret), "{secret}");
    }
    assert!(wire.contains("visible"));
}
#[test]
fn byte_bounds_preserve_utf8_and_mark_loss_without_sending_private_fields() {
    let items = vec![json!({"role":"assistant","content":"中文🙂".repeat(30000),"_private":"secret"})];
    let selection: Selection = serde_json::from_value(json!({"max_bytes":1024})).unwrap();
    let view = project(&items, &selection).unwrap();
    assert!(serde_json::to_vec(&view).unwrap().len() <= 1024);
    assert_eq!(view["truncated"], true);
    assert!(!view.to_string().contains("secret"));
    assert!(!view["messages"].as_array().unwrap().is_empty());
}
#[test]
fn selection_and_prompt_configuration_fail_closed() {
    for value in [
        json!({"roles":["system"]}),
        json!({"roles":[]}),
        json!({"roles":["user","user"]}),
        json!({"max_messages":0}),
        json!({"max_bytes":1}),
        json!({"start":3,"end":2}),
    ] {
        let selection = serde_json::from_value::<Selection>(value);
        assert!(selection.is_err() || project(&history(), &selection.unwrap()).is_err());
    }
    assert!(serde_json::from_value::<AnalysisSpec>(json!({"prompt":"x","selection":{},"api_key":"secret"})).is_err());
    assert!(request(" ", &json!({"messages":[]}), &json!([])).is_err());
    assert!(request(&"x".repeat(32001), &json!({"messages":[]}), &json!([])).is_err());
}
#[test]
fn analysis_is_untrusted_text_not_replayed_tool_messages_and_requires_complete_result() {
    let view = project(&history(), &Selection::default()).unwrap();
    let input = request("Review evidence only", &view, &json!([])).unwrap();
    assert_eq!(input.len(), 2);
    assert_eq!(input[0]["role"], "system");
    assert_eq!(input[1]["role"], "user");
    assert!(input[1]["content"].as_str().unwrap().contains("Untrusted session transcript"));
    assert!(!serde_json::to_string(&input).unwrap().contains("HOST_SYSTEM_NOT_FOR_ANALYSIS"));
    let good = json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":"Review report"}}]});
    assert_eq!(response_text(&good).unwrap(), "Review report");
    for reason in ["length", "tool_calls", "error"] {
        let mut bad = good.clone();
        bad["choices"][0]["finish_reason"] = json!(reason);
        assert!(response_text(&bad).is_err());
    }
    let mut bad = good.clone();
    bad["choices"][0]["message"]["tool_calls"] = json!([{"id":"bad","function":{"name":"write","arguments":"{}"}}]);
    assert!(response_text(&bad).is_err());
    let mut bad = good;
    bad["choices"][0]["message"]["content"] = json!(" ");
    assert!(response_text(&bad).is_err());
}
