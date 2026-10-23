#![cfg(feature = "wasm")]
use i::browser::{plugins::PluginRegistry, provider::BrowserConfig};
use i_wasm_protocol::{Manifest, public_tool_name};
use serde_json::json;
fn descriptors() -> String {
    let m:Manifest=serde_json::from_value(json!({"format_version":1,"id":"example.count","name":"Count","version":"1.0.0","runtime":"harness-wasm-v1","entry":"plugin.wasm","sha256":"a".repeat(64),"permissions":{"workspace":"read","storage":false},"tools":[{"name":"count","description":"Count words","parameters":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}}]})).unwrap();
    serde_json::to_string(&m.descriptors()).unwrap()
}
#[test]
fn plugin_schemas_are_public_and_shared_by_both_provider_protocols() {
    let registry = PluginRegistry::from_json(&descriptors()).unwrap();
    let name = public_tool_name("example.count", "count");
    for protocol in ["openai", "anthropic"] {
        let config: BrowserConfig = serde_json::from_value(
            json!({"base_url":"https://api.example","model":"fixture","protocol":protocol,"api_key":"SECRET"}),
        )
        .unwrap();
        let body = config.body_with_tools(&[json!({"role":"user","content":"count"})], &registry.schemas()).unwrap();
        assert!(body["tools"].as_array().unwrap().iter().any(|t| t["name"] == name || t["function"]["name"] == name));
        assert!(!body.to_string().contains("SECRET"));
        assert!(!body.to_string().contains("digest"));
        assert!(!body.to_string().contains("permissions"));
    }
    assert!(registry.validate_call(&name, &json!({"path":"notes.txt"})).is_ok());
    assert!(registry.validate_call(&name, &json!({})).is_err());
    assert!(registry.validate_call("shell", &json!({})).is_err());
}
#[test]
fn plugin_registry_rejects_mutated_names_duplicate_tools_and_unknown_fields() {
    let mut tools: serde_json::Value = serde_json::from_str(&descriptors()).unwrap();
    tools[0]["publicName"] = json!("write");
    assert!(PluginRegistry::from_json(&tools.to_string()).is_err());
    let tools: serde_json::Value = serde_json::from_str(&descriptors()).unwrap();
    assert!(PluginRegistry::from_json(&json!([tools[0], tools[0]]).to_string()).is_err());
}
