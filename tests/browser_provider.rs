use i::browser::provider::BrowserConfig;
use serde_json::json;

#[test]
fn request_preserves_history_and_excludes_credentials() {
    let cfg: BrowserConfig =
        serde_json::from_value(json!({"base_url":"http://localhost:1234/v1/","model":"test","api_key":"SECRET"}))
            .unwrap();
    assert_eq!(cfg.endpoint().unwrap(), "http://localhost:1234/v1/chat/completions");
    let body = cfg.body(&[json!({"role":"assistant","content":null,"reasoning_content":"opaque"})]).unwrap();
    assert_eq!(body["messages"][0]["reasoning_content"], "opaque");
    assert!(!body.to_string().contains("SECRET"));
    assert!(!body["tools"].as_array().unwrap().iter().any(|t| t["function"]["name"] == "shell"));
}
#[test]
fn openai_outbound_history_removes_provider_private_metadata_without_mutating_history() {
    let cfg: BrowserConfig =
        serde_json::from_value(json!({"base_url":"https://api.example/v1","model":"test"})).unwrap();
    let history = vec![
        json!({"role":"assistant","content":"public text","reasoning_content":"public reasoning","_anthropic_content":[{"type":"thinking","thinking":"private reasoning","signature":"private signature"}],"_future_provider_metadata":{"secret":"private"}}),
    ];
    let body = cfg.body(&history).unwrap();
    assert_eq!(
        body["messages"][0],
        json!({"role":"assistant","content":"public text","reasoning_content":"public reasoning"})
    );
    assert!(history[0].get("_anthropic_content").is_some());
    assert!(!body.to_string().contains("private"));
}
#[test]
fn browser_configuration_rejects_unsupported_protocols_and_invalid_limits() {
    for changes in [
        json!({"protocol":"responses"}),
        json!({"reasoning_effort":"ultra"}),
        json!({"max_tokens":0}),
        json!({"base_url":"https://user:pass@example.test"}),
        json!({"base_url":"javascript:bad"}),
    ] {
        let mut value = json!({"base_url":"https://example.test/v1","model":"test","api_key":"key"});
        value.as_object_mut().unwrap().extend(changes.as_object().unwrap().clone());
        let cfg: BrowserConfig = serde_json::from_value(value).unwrap();
        assert!(cfg.validate().is_err());
    }
}

#[test]
fn anthropic_roundtrip_preserves_opaque_blocks_and_native_tools() {
    let cfg: BrowserConfig =
        serde_json::from_value(json!({"base_url":"https://api.example","model":"test","protocol":"anthropic"}))
            .unwrap();
    assert_eq!(cfg.endpoint().unwrap(), "https://api.example/v1/messages");
    let blocks = json!([{"type":"thinking","thinking":"private","signature":"opaque"},{"type":"tool_use","id":"t1","name":"write","input":{"path":"a","content":"ok"}}]);
    let normalized = cfg
        .normalize_response(json!({"type":"message","role":"assistant","content":blocks,"stop_reason":"tool_use"}))
        .unwrap();
    let body = cfg
        .body(&[
            json!({"role":"system","content":"instructions"}),
            json!({"role":"user","content":"task"}),
            normalized["choices"][0]["message"].clone(),
            json!({"role":"tool","tool_call_id":"t1","content":"ok"}),
        ])
        .unwrap();
    assert_eq!(body["system"][0]["text"], "instructions");
    assert_eq!(body["messages"][1]["content"], blocks);
    assert_eq!(body["messages"][2]["content"][0]["tool_use_id"], "t1");
    assert!(body["tools"][0]["input_schema"].is_object());
    assert!(body["max_tokens"].is_number());
}

#[test]
fn anthropic_rejects_incomplete_or_unsupported_batches_before_dispatch() {
    let cfg: BrowserConfig = serde_json::from_value(
        json!({"base_url":"https://api.example/v1/messages","model":"test","protocol":"anthropic"}),
    )
    .unwrap();
    for (reason, blocks) in [
        ("max_tokens", json!([])),
        ("end_turn", json!([{"type":"tool_use","id":"a","name":"write","input":{}}])),
        ("tool_use", json!([])),
        ("tool_use", json!([{"type":"tool_use","id":"a","name":"shell","input":{}}])),
        (
            "tool_use",
            json!([{"type":"tool_use","id":"a","name":"write","input":{}},{"type":"tool_use","id":"a","name":"read","input":{}}]),
        ),
    ] {
        assert!(
            cfg.normalize_response(json!({"type":"message","role":"assistant","content":blocks,"stop_reason":reason}))
                .is_err()
        );
    }
}
#[test]
fn deepseek_thinking_controls_are_explicit_and_not_sent_to_other_providers() {
    for effort in ["default", "none", "low", "high", "max"] {
        let mut config: i::browser::provider::BrowserConfig = serde_json::from_value(json!({
            "base_url":"https://api.deepseek.com", "model":"deepseek-flash", "max_tokens":393216, "reasoning_effort":effort
        })).unwrap();
        let body = config.body(&[]).unwrap();
        assert_eq!(body["max_tokens"], 393216);
        if effort == "default" {
            assert!(body.get("thinking").is_none());
            assert!(body.get("reasoning_effort").is_none());
        } else {
            assert_eq!(body["reasoning_effort"], effort);
            assert_eq!(body["thinking"]["type"], if effort == "none" { "disabled" } else { "enabled" });
        }
        config.base_url = "https://api.other.example/v1".into();
        let body = config.body(&[]).unwrap();
        assert!(body.get("thinking").is_none());
        assert!(body.get("reasoning_effort").is_none());
    }
}
#[test]
fn deepseek_maximum_is_bounded_and_uses_documented_token_parameter() {
    let mut config: BrowserConfig = serde_json::from_value(json!({"base_url":"https://api.deepseek.com","model":"deepseek-flash","max_tokens":393216,"use_max_tokens":false})).unwrap();
    let body = config.body(&[]).unwrap();
    assert_eq!(body["max_tokens"], 393216);
    assert!(body.get("max_completion_tokens").is_none());
    config.max_tokens = 393217;
    assert!(config.validate().is_err());
}
