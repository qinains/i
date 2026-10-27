use super::*;
use std::fs;

fn event(kind: &str, data: Value, dir: &Path) -> Value {
    json!({"id":1,"event":kind,"workspace":dir,"paths":{
        "config_dir":dir.join("config"),"data_dir":dir.join("data"),
        "cache_dir":dir.join("cache"),"runtime_dir":dir.join("runtime")},"data":data})
}

#[test]
fn reflection_prepares_only_prompt_from_the_selected_transcript() {
    let dir = tempfile::tempdir().unwrap();
    let data = json!({"name":"reflection","model":"fixture","transcript":{
        "messages":[{"index":2,"role":"user","content":"Please reflect"}],"truncated":false}});
    let result = handle("reflection", event("analysis_prepare", data, dir.path()), &[]).unwrap();
    let prompt = result["prompt"].as_str().unwrap();
    assert!(prompt.contains("facts") && prompt.contains("inferences") && prompt.contains("language"));
    assert!(prompt.contains("untrusted") && prompt.contains("Do not execute"));
    assert!(result.get("transcript").is_none());
    assert!(result.get("messages").is_none());
    assert!(result.get("history").is_none());
}

#[test]
fn reflection_preserves_only_summary_and_optional_state_in_results() {
    let dir = tempfile::tempdir().unwrap();
    let data = json!({"summary":"Observed failure; verify next step.","state":{"sample":1},"history":["hidden"]});
    let result = handle("reflection", event("analysis_result", data, dir.path()), &[]).unwrap();
    assert_eq!(result, json!({"summary":"Observed failure; verify next step.","state":{"sample":1}}));
    for summary in [json!(""), json!("  "), json!(42), json!("x".repeat(128_001))] {
        assert!(handle("reflection", event("analysis_result", json!({"summary":summary}), dir.path()), &[]).is_err());
    }
}

#[test]
fn reflection_supports_prompt_file_and_explicit_disable_without_automatic_actions() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("prompt.txt"), "Custom reflection; use evidence.").unwrap();
    let config = dir.path().join("reflection.json");
    fs::write(&config, r#"{"enabled":true,"prompt":"prompt.txt"}"#).unwrap();
    let args = vec!["--config".into(), config.to_str().unwrap().into()];
    let data = json!({"name":"reflection","transcript":{"messages":[]},"model":"fixture"});
    let result = handle("reflection", event("analysis_prepare", data.clone(), dir.path()), &args).unwrap();
    assert!(result["prompt"].as_str().unwrap().contains("Custom reflection; use evidence."));
    fs::write(&config, r#"{"enabled":false}"#).unwrap();
    assert!(handle("reflection", event("analysis_prepare", data, dir.path()), &args).is_err());
    assert!(handle("reflection", event("before_request", json!({}), dir.path()), &[]).is_err());
}

#[test]
fn reflection_requires_a_projection_and_rejects_legacy_history_input() {
    let dir = tempfile::tempdir().unwrap();
    for data in [
        json!({}),
        json!({"name":"reflection","transcript":[]}),
        json!({"name":"other","transcript":{}}),
        json!({"name":"reflection","transcript":{},"history":["raw"]}),
        json!({"name":"reflection","transcript":{},"api_key":"raw"}),
    ] {
        assert!(handle("reflection", event("analysis_prepare", data, dir.path()), &[]).is_err());
    }
}

#[test]
fn reflection_canonical_spec_and_manifests_share_complete_selection() {
    let spec: Value = serde_json::from_str(include_str!("../../reflection-spec.json")).unwrap();
    assert_eq!(spec.as_object().unwrap().len(), 2);
    assert!(!spec["prompt"].as_str().unwrap().trim().is_empty());
    assert_eq!(spec["selection"].as_object().unwrap().len(), 5);
    for source in [include_str!("../../reflection.json"), include_str!("../../full.json")] {
        let manifest: Value = serde_json::from_str(source).unwrap();
        assert_eq!(manifest["analyses"][0]["selection"], spec["selection"]);
    }
}

#[test]
fn reflection_rejects_invalid_configuration_and_prompt_files() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("reflection.json");
    let args = vec!["--config".into(), config.to_str().unwrap().into()];
    for value in [
        json!([]),
        json!({"enabled":"yes"}),
        json!({"prompt":false}),
        json!({"prompt":"missing.txt"}),
        json!({"automatic":true}),
    ] {
        fs::write(&config, value.to_string()).unwrap();
        assert!(
            handle(
                "reflection",
                event("analysis_prepare", json!({"name":"reflection","transcript":{}}), dir.path()),
                &args
            )
            .is_err()
        );
    }
    for content in [vec![], vec![b' '; 10], vec![0xff], vec![b'x'; 128_001]] {
        fs::write(dir.path().join("prompt.txt"), content).unwrap();
        fs::write(&config, r#"{"prompt":"prompt.txt"}"#).unwrap();
        assert!(
            handle(
                "reflection",
                event("analysis_prepare", json!({"name":"reflection","transcript":{}}), dir.path()),
                &args
            )
            .is_err()
        );
    }
}

#[test]
fn reflection_manifest_profiles_bind_and_disable_analysis_independently() {
    let dir = tempfile::tempdir().unwrap();
    let paths: paths::AppPaths =
        serde_json::from_value(event("unused", json!({}), dir.path())["paths"].clone()).unwrap();
    fs::create_dir_all(&paths.config_dir).unwrap();
    for profile in ["standard", "full", "reflection"] {
        let value = desktop::manifest(&paths, Some(profile)).unwrap();
        let entry = &value["analyses"][0];
        assert_eq!(entry["name"], "reflection");
        assert_eq!(
            entry["selection"],
            json!({"roles":["user","assistant","tool"],"start":0,"end":null,"max_messages":40,"max_bytes":48000})
        );
        assert!(Path::new(entry["exec"][0].as_str().unwrap()).is_absolute());
    }
    let exec = std::env::current_exe().unwrap();
    fs::write(
        paths.config_dir.join("bindings.json"),
        json!({"reflection":{"exec":[exec,"custom-reflector"],"cwd":dir.path()}}).to_string(),
    )
    .unwrap();
    let value = desktop::manifest(&paths, Some("standard")).unwrap();
    assert_eq!(value["analyses"][0]["exec"][1], "custom-reflector");
    assert_eq!(value["analyses"][0]["cwd"], json!(dir.path()));
    fs::write(paths.config_dir.join("plugins.json"), r#"{"disabled":["reflection"]}"#).unwrap();
    assert_eq!(desktop::manifest(&paths, Some("full")).unwrap()["analyses"], json!([]));
    assert!(desktop::manifest(&paths, Some("approval")).is_ok());
}
