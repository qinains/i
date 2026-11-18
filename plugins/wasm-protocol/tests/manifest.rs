use i_wasm_protocol::{public_tool_name, validate_arguments, validate_manifest};
use serde_json::{Value, json};

fn manifest() -> Value {
    json!({"format_version":1,"id":"example.word-count","name":"Word count","version":"1.0.0","runtime":"harness-wasm-v1","entry":"plugin.wasm","sha256":"a".repeat(64),"permissions":{"workspace":"read","storage":false},"tools":[{"name":"count_words","description":"Count words","parameters":{"type":"object","properties":{"path":{"type":"string","minLength":1}},"required":["path"],"additionalProperties":false}}]})
}
#[test]
fn valid_manifest_and_arguments() {
    let m = manifest();
    assert!(validate_manifest(&m).is_ok());
    let schema = &m["tools"][0]["parameters"];
    assert!(validate_arguments(schema, &json!({"path":"notes.txt"})).is_ok());
    for bad in [json!({}), json!({"path":""}), json!({"path":3}), json!({"path":"x","extra":1})] {
        assert!(validate_arguments(schema, &bad).is_err());
    }
}
#[test]
fn rejects_manifest_names_versions_fields_permissions_and_duplicate_tools() {
    for (key, bad) in [
        ("id", json!("../escape")),
        ("id", json!("A")),
        ("version", json!("1.0")),
        ("version", json!("01.0.0")),
        ("version", json!("1.0.0-beta")),
        ("runtime", json!("javascript")),
        ("entry", json!("other.wasm")),
        ("format_version", json!(2)),
        ("sha256", json!("x")),
    ] {
        let mut m = manifest();
        m[key] = bad;
        assert!(validate_manifest(&m).is_err(), "accepted {key}");
    }
    let mut m = manifest();
    m["permissions"]["network"] = json!(true);
    assert!(validate_manifest(&m).is_err());
    let mut m = manifest();
    m["exec"] = json!(["sh"]);
    assert!(validate_manifest(&m).is_err());
    let mut m = manifest();
    m["tools"] = json!(vec![m["tools"][0].clone(); 17]);
    assert!(validate_manifest(&m).is_err());
    let mut m = manifest();
    m["tools"] = json!([m["tools"][0], m["tools"][0]]);
    assert!(validate_manifest(&m).is_err());
}
#[test]
fn schema_rejects_unknown_keywords_refs_and_unbounded_complexity() {
    for schema in [
        json!({"type":"object","$ref":"https://outside.invalid"}),
        json!({"type":"object","allOf":[]}),
        json!({"type":"object","patternProperties":{}}),
        json!({"type":"object","required":["missing"]}),
        json!({"type":"array","items":{"type":"string"}}),
    ] {
        let mut m = manifest();
        m["tools"][0]["parameters"] = schema;
        assert!(validate_manifest(&m).is_err());
    }
    let mut nested = json!({"type":"string"});
    for _ in 0..17 {
        nested = json!({"type":"array","items":nested});
    }
    let mut m = manifest();
    m["tools"][0]["parameters"]["properties"]["path"] = nested;
    assert!(validate_manifest(&m).is_err());
    let mut m = manifest();
    m["tools"][0]["parameters"]["properties"]["path"]["enum"] = json!(vec!["x"; 257]);
    assert!(validate_manifest(&m).is_err());
}
#[test]
fn nested_schema_enforces_types_ranges_arrays_and_unicode_lengths() {
    let schema = json!({"type":"object","properties":{"n":{"type":"integer","minimum":1,"maximum":3},"a":{"type":"array","minItems":1,"maxItems":2,"items":{"type":"string","minLength":1,"maxLength":2}}},"required":["n","a"],"additionalProperties":false});
    assert!(validate_arguments(&schema, &json!({"n":2,"a":["中文"]})).is_ok());
    for bad in [json!({"n":0,"a":["x"]}), json!({"n":1.5,"a":["x"]}), json!({"n":2,"a":[]}), json!({"n":2,"a":["abc"]})]
    {
        assert!(validate_arguments(&schema, &bad).is_err());
    }
}
#[test]
fn public_namespace_is_deterministic_bounded_and_plugin_specific() {
    let name = public_tool_name("example.word-count", "count_words");
    assert_eq!(name.len(), 2 + 11 + 1 + 16);
    assert!(name.starts_with("p_count_words_"));
    assert_eq!(name, public_tool_name("example.word-count", "count_words"));
    assert_ne!(name, public_tool_name("other.word-count", "count_words"));
}
#[test]
fn protocol_frames_roundtrip_and_reject_unrecognized_fields() {
    use i_wasm_protocol::{StepInput, StepOutput, validate_schema};
    let start = json!({"type":"start","request":{"id":"call1","event":"tool","workspace_id":"default","data":{"name":"count","arguments":{}}}});
    assert_eq!(serde_json::to_value(serde_json::from_value::<StepInput>(start.clone()).unwrap()).unwrap(), start);
    let done = json!({"type":"done","output":"result"});
    assert_eq!(serde_json::to_value(serde_json::from_value::<StepOutput>(done.clone()).unwrap()).unwrap(), done);
    assert!(serde_json::from_value::<StepOutput>(json!({"type":"done","output":"x","exec":"evil"})).is_err());
    let props: serde_json::Map<String, Value> =
        (0..1025).map(|i| (format!("p{i}"), json!({"type":"boolean"}))).collect();
    assert!(validate_schema(&json!({"type":"object","properties":props})).is_err());
}
