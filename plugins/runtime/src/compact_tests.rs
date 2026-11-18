use super::compact;
use serde_json::{Value, json};
use std::fs;
fn history() -> Value {
    json!([
        {"role":"system"},{"role":"user"},{"role":"assistant"},{"role":"tool"},
        {"role":"assistant"},{"role":"user"},{"role":"assistant"},{"role":"user"}
    ])
}
#[test]
fn compact_policy_is_independent_configurable_and_preserves_complete_suffixes() {
    let data = json!({"compact":true,"history":history()});
    assert_eq!(compact::handle("compact_plan", data.clone(), ".", &[]).unwrap()["cut"], 4);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("compact.json");
    let args = vec!["--config".into(), path.to_string_lossy().into_owned()];
    for (keep, cut) in [(1, json!(7)), (4, json!(4)), (5, json!(2)), (100, json!(null))] {
        fs::write(&path, json!({"keep_recent":keep}).to_string()).unwrap();
        assert_eq!(compact::handle("compact_plan", data.clone(), ".", &args).unwrap()["cut"], cut);
    }
    fs::write(&path, r#"{"enabled":false}"#).unwrap();
    assert!(compact::handle("compact_plan", data.clone(), ".", &args).unwrap()["cut"].is_null());
    for config in
        [json!({"keep_recent":0}), json!({"keep_recent":10001}), json!({"keep_recent":"4"}), json!({"enabled":"yes"})]
    {
        fs::write(&path, config.to_string()).unwrap();
        assert!(compact::handle("compact_plan", data.clone(), ".", &args).is_err());
    }
}
#[test]
fn compact_prompt_and_image_policy_come_from_the_selected_compactor_config() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("compact.json");
    fs::write(dir.path().join("prompt.txt"), "Custom compact for {{workspace}}").unwrap();
    fs::write(&path, r#"{"prompt":"prompt.txt","images":"omit"}"#).unwrap();
    let args = vec!["--config".into(), path.to_string_lossy().into_owned()];
    let input = json!({"model":"fixture","policy":{"models":{"fixture":{"images":"keep"}}},"messages":[
        {"role":"user","content":[{"type":"image_url","image_url":{"url":"data:image/png;base64,PRIVATE"}}]}]});
    let prepared = compact::handle("before_compact", input.clone(), "/workspace", &args).unwrap();
    assert!(prepared["prompt"].as_str().unwrap().starts_with("Custom compact for /workspace"));
    assert_eq!(prepared["retained_images"], json!([]));
    assert!(!prepared.to_string().contains("PRIVATE"));
    for images in [json!(42), json!("invalid")] {
        fs::write(&path, json!({"images":images}).to_string()).unwrap();
        assert!(compact::handle("before_compact", input.clone(), ".", &args).is_err());
    }
}
