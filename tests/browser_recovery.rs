use i::agent::recovery::recover;
use serde_json::{Value, json};

fn call(id: &str) -> Value {
    json!({"id":id,"type":"function","function":{"name":"view_image","arguments":"{}"}})
}
fn image() -> Value {
    json!({"type":"image_url","image_url":{"url":"data:image/png;base64,iVBORw0KGgo="}})
}
#[test]
fn finished_images_follow_all_results_and_missing_calls_are_not_replayed() {
    let history = vec![json!({"role":"system","content":"test"})];
    let journal = json!({"assistant":{"role":"assistant","content":null,"tool_calls":[call("a"),call("b"),call("c")]},"calls":[{"id":"b","status":"pending","result":{"images":[image()]}},{"id":"a","status":"finished","result":{"output":"Image attached.","images":[image()]}}]});
    let recovered = recover(&history, Some(&journal)).unwrap();
    assert_eq!(
        recovered.iter().map(|v| v["role"].as_str().unwrap()).collect::<Vec<_>>(),
        ["system", "assistant", "tool", "tool", "tool", "user"]
    );
    assert_eq!(recovered[5]["content"], json!([{"type":"text","text":"Image from tool \"a\""},image()]));
    assert!(recovered[4]["content"].as_str().unwrap().contains("not executed"));
    assert_eq!(recover(&history, None).unwrap(), history);
}
#[test]
fn malformed_finished_results_and_images_fail_closed() {
    let history = vec![json!({"role":"system","content":"test"})];
    for result in [
        json!({}),
        json!({"output":1}),
        json!({"output":"x","images":{}}),
        json!({"output":"x","images":[{"type":"image_url","image_url":{"url":"https://example.invalid/a.png"}}]}),
        json!({"output":"x","images":[{"type":"image_url","image_url":{"url":"data:image/jpeg;base64,iVBORw0KGgo="}}]}),
        json!({"output":"x","images":vec![image();9]}),
    ] {
        let journal = json!({"assistant":{"role":"assistant","tool_calls":[call("a")]},"calls":[{"id":"a","status":"finished","result":result}]});
        assert!(recover(&history, Some(&journal)).is_err());
    }
}
#[test]
fn duplicate_unknown_and_invalid_journal_calls_are_rejected() {
    let history = vec![json!({"role":"system","content":"test"})];
    for calls in [
        json!([{"id":"unknown","status":"pending"}]),
        json!([{"id":"a","status":"pending"},{"id":"a","status":"started"}]),
        json!([{"id":"a","status":"invalid"}]),
    ] {
        let journal = json!({"assistant":{"role":"assistant","tool_calls":[call("a")]},"calls":calls});
        assert!(recover(&history, Some(&journal)).is_err());
    }
    assert!(recover(&[json!({"role":"user","content":"no system"})], None).is_err());
}

#[test]
fn image_limits_are_per_batch_and_per_file_and_base64_is_canonical() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let history = vec![json!({"role":"system","content":"test"})];
    let mut journal = json!({"assistant":{"role":"assistant","tool_calls":[call("a"),call("b")]},"calls":[{"id":"b","status":"finished","result":{"output":"b","images":[image()]}},{"id":"a","status":"finished","result":{"output":"a","images":[image()]}}]});
    let recovered = recover(&history, Some(&journal)).unwrap();
    assert_eq!(recovered[4]["content"][0]["text"], "Image from tool \"a\"");
    assert_eq!(recovered[4]["content"][2]["text"], "Image from tool \"b\"");
    journal["calls"][0]["result"]["images"] = json!(vec![image(); 8]);
    assert!(recover(&history, Some(&journal)).is_err());
    journal["calls"][0]["result"]["images"] =
        json!([{"type":"image_url","image_url":{"url":"data:image/png;base64,iVBORw0KGgo"}}]);
    assert!(recover(&history, Some(&journal)).is_err());
    let mut bytes = vec![0; 8 * 1024 * 1024 + 1];
    bytes[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    journal["calls"][0]["result"]["images"] =
        json!([{"type":"image_url","image_url":{"url":format!("data:image/png;base64,{}",STANDARD.encode(bytes))}}]);
    assert!(recover(&history, Some(&journal)).is_err());
}
