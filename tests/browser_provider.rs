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
fn browser_configuration_rejects_unsupported_or_unbounded_requests() {
    for changes in [
        json!({"protocol":"responses"}),
        json!({"max_steps":33}),
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
