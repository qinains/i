use i::browser::provider::BrowserConfig;
use serde_json::{Value, json};

fn config(protocol: &str) -> BrowserConfig {
    serde_json::from_value(json!({"base_url":"https://api.example/v1","model":"fixture","api_key":"SECRET","protocol":protocol,"stream":true})).unwrap()
}

#[test]
fn analysis_bodies_have_no_tools_and_do_not_mutate_the_supplied_history() {
    let history = vec![
        json!({"role":"system","content":"Analyze only the supplied data"}),
        json!({"role":"user","content":"Selected conversation data"}),
    ];
    let before = history.clone();
    for protocol in ["openai", "responses", "anthropic"] {
        let body = config(protocol).analysis_body(&history).unwrap();
        for field in ["tools", "tool_choice", "parallel_tool_calls", "functions", "function_call", "max_tool_calls"] {
            assert!(body.get(field).is_none(), "{protocol} advertises {field}");
        }
        assert_eq!(body["model"], "fixture");
        assert_eq!(body["stream"], true);
        assert!(!body.to_string().contains("SECRET"));
        if protocol == "responses" {
            assert_eq!(body["store"], false);
            assert!(body["input"].is_array());
        } else {
            assert!(body["messages"].is_array());
        }
    }
    assert_eq!(history, before);
}

#[test]
fn analysis_bodies_keep_connection_transport_auth_and_config_unchanged() {
    let mut cfg = config("responses");
    cfg.transport = "plugin".into();
    cfg.base_url = "http://127.0.0.1:12456/v1/responses".into();
    cfg.api_key = "a".repeat(64);
    let before = cfg.api_key.clone();
    let body = cfg.analysis_body(&[json!({"role":"user","content":"analyze"})]).unwrap();
    assert!(body.get("tools").is_none());
    assert!(body.get("api_key").is_none());
    assert_eq!(cfg.api_key, before);
    assert_eq!(cfg.transport, "plugin");
    assert_eq!(cfg.endpoint().unwrap(), "http://127.0.0.1:12456/v1/responses");
}

#[test]
fn analysis_rejects_any_tool_calls_and_truncated_responses() {
    for protocol in ["openai", "responses", "anthropic"] {
        let cfg = config(protocol);
        let (valid, tool_call, incomplete) = match protocol {
            "openai" => (
                json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":"analysis"}}]}),
                json!({"choices":[{"finish_reason":"tool_calls","message":{"role":"assistant","content":"","tool_calls":[{"id":"c","type":"function","function":{"name":"write","arguments":"{}"}}]}}]}),
                json!({"choices":[{"finish_reason":"length","message":{"role":"assistant","content":"partial"}}]}),
            ),
            "responses" => (
                json!({"object":"response","status":"completed","output":[{"id":"m","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"analysis"}]}]}),
                json!({"object":"response","status":"completed","output":[{"id":"f","type":"function_call","status":"completed","call_id":"c","name":"write","arguments":"{}"}]}),
                json!({"object":"response","status":"incomplete","output":[]}),
            ),
            _ => (
                json!({"type":"message","role":"assistant","stop_reason":"end_turn","content":[{"type":"text","text":"analysis"}]}),
                json!({"type":"message","role":"assistant","stop_reason":"tool_use","content":[{"type":"tool_use","id":"c","name":"write","input":{}}]}),
                json!({"type":"message","role":"assistant","stop_reason":"max_tokens","content":[{"type":"text","text":"partial"}]}),
            ),
        };
        assert_eq!(cfg.analysis_response_text(valid).unwrap(), "analysis");
        assert!(cfg.analysis_response_text(tool_call).is_err(), "{protocol} accepted tools");
        assert!(cfg.analysis_response_text(incomplete).is_err(), "{protocol} accepted truncation");
    }
    assert!(config("openai").analysis_response_text(Value::Null).is_err());
    assert!(config("openai").analysis_response_text(json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":"answer","tool_calls":[{"id":"c","type":"function","function":{"name":"write","arguments":"{}"}}]}}]})).is_err());
    assert!(config("openai").analysis_response_text(json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":"answer","function_call":{"name":"write","arguments":"{}"}}}]})).is_err());
    assert!(config("openai").analysis_response_text(json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":" ","reasoning_content":"hidden"}}]})).is_err());
}

#[test]
fn analysis_projects_only_selected_visible_history_without_mutating_it() {
    let history = vec![
        json!({"role":"system","content":"PRIVATE_SYSTEM"}),
        json!({"role":"user","content":"outside selection"}),
        json!({"role":"assistant","content":"visible answer","reasoning_content":"PRIVATE_REASONING","_responses_output":[{"encrypted_content":"PRIVATE_ENCRYPTED"}]}),
        json!({"role":"tool","content":"outside role selection","tool_call_id":"c"}),
    ];
    let before = history.clone();
    let selection: i::analysis::Selection =
        serde_json::from_value(json!({"roles":["assistant"],"start":0,"end":null,"max_messages":20,"max_bytes":4096}))
            .unwrap();
    let projected = i::analysis::project(&history, &selection).unwrap().to_string();
    assert!(projected.contains("visible answer"));
    for hidden in
        ["PRIVATE_SYSTEM", "PRIVATE_REASONING", "PRIVATE_ENCRYPTED", "outside selection", "outside role selection"]
    {
        assert!(!projected.contains(hidden));
    }
    assert_eq!(history, before);
}
