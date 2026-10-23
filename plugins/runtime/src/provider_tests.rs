use super::*;
fn request(kind: &str, history: Value) -> Value {
    json!({"version":1,"protocol":kind,"base_url":"https://provider.invalid/v1","model":"fixture",
        "max_tokens":128,"history":history,"tools":[],"plugin_tools":[],"extra_body":{},"compact":false})
}
#[test]
fn provider_adapters_own_routes_auth_images_and_token_fields() {
    let history = json!([{"role":"system","content":"fixed"},{"role":"user","content":[
        {"type":"text","text":"look"},{"type":"image_url","image_url":{"url":"data:image/png;base64,abcd"}}]}]);
    for (kind, route, token_field, auth) in [
        ("openai", "chat/completions", "max_completion_tokens", "authorization"),
        ("deepseek", "chat/completions", "max_tokens", "authorization"),
        ("anthropic", "messages", "max_tokens", "x-api-key"),
        ("responses", "responses", "max_output_tokens", "authorization"),
    ] {
        let prepared = provider::handle("provider_prepare", request(kind, history.clone())).unwrap();
        assert_eq!(prepared["url"], format!("https://provider.invalid/v1/{route}"));
        assert_eq!(prepared["auth"]["header"], auth);
        assert_eq!(prepared["body"][token_field], 128);
        if kind == "anthropic" {
            assert_eq!(prepared["headers"]["anthropic-version"], "2023-06-01");
            assert_eq!(prepared["body"]["messages"][0]["content"][1]["source"]["data"], "abcd");
        } else if kind == "responses" {
            assert_eq!(prepared["body"]["store"], false);
            assert_eq!(prepared["body"]["input"][1]["content"][1]["type"], "input_image");
        } else {
            assert_eq!(prepared["body"]["messages"], history);
        }
    }
}
#[test]
fn responses_provider_preserves_opaque_reasoning_and_refuses_incomplete_output() {
    let output = json!([{"type":"reasoning","id":"r1","encrypted_content":"opaque","summary":[]},
        {"type":"function_call","id":"fc1","call_id":"c1","name":"read","arguments":"{ \"path\": \"a\" }"}]);
    let response = json!({"status":"completed","output":output});
    let parsed =
        provider::handle("provider_parse", json!({"version":1,"protocol":"responses","response":response})).unwrap();
    let history = json!([{"role":"user","content":"read"},parsed["response"]["choices"][0]["message"],
        {"role":"tool","tool_call_id":"c1","content":"result"}]);
    let prepared = provider::handle("provider_prepare", request("responses", history)).unwrap();
    assert_eq!(prepared["body"]["input"][1], output[0]);
    assert_eq!(prepared["body"]["input"][2], output[1]);
    assert_eq!(prepared["body"]["input"][3]["call_id"], "c1");
    assert!(
        provider::handle(
            "provider_parse",
            json!({"version":1,"protocol":"responses",
        "response":{"status":"incomplete","output":output}})
        )
        .is_err()
    );
}
#[test]
fn usage_anchors_do_not_cross_provider_scopes_with_the_same_model_name() {
    let history = json!([{"role":"user","content":"question"}]);
    let scope = json!(["https://one.invalid", "openai"]);
    let prepared = budget::handle(
        "before_request",
        json!({"history":history,
        "body":{"model":"fixture"},"provider_scope":scope}),
    )
    .unwrap();
    let mut messages = history.as_array().unwrap().clone();
    messages.push(json!({"role":"assistant","content":"answer","_usage":{
        "tokens":{"prompt_tokens":100,"completion_tokens":10},"anchor":prepared["budget_anchor"]}}));
    let mut data = json!({"history":messages,"model":"fixture","provider_scope":scope,
        "max_tokens":100,"policy":{"context_window":10000}});
    assert_eq!(budget::budget(data.clone()).unwrap()["source"], "usage+estimate");
    data["provider_scope"] = json!(["https://two.invalid", "anthropic"]);
    assert_eq!(budget::budget(data.clone()).unwrap()["source"], "estimate");
    data["provider_scope"] = scope;
    data["history"][1]["_usage"]["anchor"].as_object_mut().unwrap().remove("scope");
    assert_eq!(budget::budget(data).unwrap()["source"], "estimate");
}
#[test]
fn provider_config_rejects_unknown_adapters_versions_and_reserved_overrides() {
    assert!(provider::handle("provider_prepare", request("unknown", json!([]))).is_err());
    let mut data = request("deepseek", json!([]));
    data["version"] = json!(2);
    assert!(provider::handle("provider_prepare", data).is_err());
    for key in ["messages", "tools", "stream", "previous_response_id", "model", "max_tokens", "store"] {
        let mut data = request("openai", json!([]));
        data["extra_body"][key] = json!(true);
        assert!(provider::handle("provider_prepare", data).is_err(), "accepted {key}");
    }
}
