use super::*;
#[path = "provider_lifecycle_tests.rs"]
mod lifecycle_tests;
fn input() -> Value {
    json!({"version":1,"protocol":"deepseek","base_url":"https://example.invalid/v1",
        "credential_scope":"credential-A","model":"fixture","max_tokens":128,"extra_body":{},
        "tools":[],"plugin_tools":[],"history":[{"role":"user","content":[
            {"type":"image_url","image_url":{"url":format!("data:image/png;base64,{}",STANDARD.encode(b"\x89PNG\r\n\x1a\nfixture"))}}]}]})
}
fn finish(root: &Path, input: &Value, plan: &Value) -> Value {
    let mut data = input.clone();
    data["operation_state"] = plan["state"].clone();
    data["operation_result"] = json!({"id":"file-api-fixture","expires_at":now().unwrap()+3600});
    files(data, "account", Some(3600), root).unwrap()
}
#[test]
fn multipart_references_are_stable_scoped_and_preserve_snapshots() {
    let root = tempfile::tempdir().unwrap();
    let data = input();
    let plan = files(data.clone(), "account", Some(3600), root.path()).unwrap();
    assert_eq!(plan["operation"]["url"], "https://example.invalid/files");
    let bytes = STANDARD.decode(plan["operation"]["body_base64"].as_str().unwrap()).unwrap();
    let wire = String::from_utf8_lossy(&bytes);
    assert!(wire.contains("name=\"purpose\"\r\n\r\nuser_data"));
    assert!(wire.contains("name=\"expires_after[seconds]\"\r\n\r\n3600"));
    assert!(bytes.windows(8).any(|w| w == b"\x89PNG\r\n\x1a\n"));
    let first = finish(root.path(), &data, &plan);
    let again = files(data.clone(), "account", Some(3600), root.path()).unwrap();
    assert_eq!(first, again);
    assert_eq!(first["body"]["messages"][0]["content"][0], json!({"type":"file","file_id":"file-api-fixture"}));
    assert_eq!(data, input());
    let mut alias = data.clone();
    alias["base_url"] = json!("https://EXAMPLE.invalid:443");
    assert!(files(alias, "account", Some(3600), root.path()).unwrap().get("operation").is_none());
    for change in ["key", "account", "endpoint", "bytes"] {
        let mut changed = data.clone();
        let account = if change == "account" { "other" } else { "account" };
        if change == "key" {
            changed["credential_scope"] = json!("credential-B");
        }
        if change == "endpoint" {
            changed["base_url"] = json!("https://other.invalid");
        }
        if change == "bytes" {
            changed["history"][0]["content"][0]["image_url"]["url"] =
                json!(format!("data:image/png;base64,{}", STANDARD.encode(b"\x89PNG\r\n\x1a\nchanged")));
        }
        assert!(files(changed, account, Some(3600), root.path()).unwrap().get("operation").is_some(), "{change}");
    }
    let stored = fs::read_to_string(root.path().join("provider-files.json")).unwrap();
    assert!(!stored.contains("data:image") && !stored.contains("credential-A"));
}
#[test]
fn unknown_uploads_bad_responses_and_expiry_never_automatically_reupload() {
    for fault in ["lost", "missing_id", "missing_expiry", "bad_expiry", "nonce"] {
        let root = tempfile::tempdir().unwrap();
        let data = input();
        let plan = files(data.clone(), "account", Some(3600), root.path()).unwrap();
        let mut bad = data.clone();
        bad["operation_state"] = plan["state"].clone();
        bad["operation_result"] = json!({"id":"file-api-fixture","expires_at":now().unwrap()+3600});
        match fault {
            "lost" => {
                bad.as_object_mut().unwrap().remove("operation_result");
            }
            "missing_id" => bad["operation_result"]["id"] = Value::Null,
            "missing_expiry" => bad["operation_result"]["expires_at"] = Value::Null,
            "bad_expiry" => bad["operation_result"]["expires_at"] = json!(u64::MAX),
            _ => bad["operation_state"]["nonce"] = json!("wrong"),
        }
        assert!(files(bad, "account", Some(3600), root.path()).is_err(), "{fault}");
        assert!(files(data, "account", Some(3600), root.path()).unwrap_err().to_string().contains("pending/unknown"));
    }
    let root = tempfile::tempdir().unwrap();
    let data = input();
    let plan = files(data.clone(), "account", Some(3600), root.path()).unwrap();
    finish(root.path(), &data, &plan);
    let path = root.path().join("provider-files.json");
    let mut db: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    db["entries"][plan["state"]["key"].as_str().unwrap()]["expires_at"] = json!(1);
    state::atomic(&path, &db).unwrap();
    assert!(files(data, "account", Some(3600), root.path()).unwrap_err().to_string().contains("expired"));
}
#[test]
fn registry_forget_requires_explicit_ack_and_upload_limit_leaves_no_pending_entry() {
    let root = tempfile::tempdir().unwrap();
    let mut data = input();
    data["operations_remaining"] = json!(0);
    assert!(files(data, "account", Some(3600), root.path()).is_err());
    assert!(!root.path().join("provider-files.json").exists());
    let plan = files(input(), "account", Some(3600), root.path()).unwrap();
    let key = plan["state"]["key"].as_str().unwrap().to_owned();
    assert!(registry(root.path(), &["--forget".into(), key.clone()]).is_err());
    let mut args = vec!["--forget".into(), key.clone(), "--confirm".into()];
    assert!(registry(root.path(), &args).is_err());
    args.push("--ack-unknown".into());
    assert!(registry(root.path(), &args).unwrap()["entries"].get(&key).is_none());
    assert!(files(input(), "account", Some(3600), root.path()).unwrap().get("operation").is_some());
}
#[test]
fn concurrent_preparations_schedule_only_one_upload() {
    let root = tempfile::tempdir().unwrap();
    std::thread::scope(|scope| {
        let first = scope.spawn(|| files(input(), "account", Some(3600), root.path()));
        let second = scope.spawn(|| files(input(), "account", Some(3600), root.path()));
        let results = [first.join().unwrap(), second.join().unwrap()];
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert!(results.iter().find_map(|r| r.as_ref().err()).unwrap().to_string().contains("pending/unknown"));
    });
}
#[test]
fn files_config_requires_explicit_host_consent_and_valid_options() {
    let root = tempfile::tempdir().unwrap();
    let paths = AppPaths {
        config_dir: root.path().join("config"),
        data_dir: root.path().join("data"),
        cache_dir: root.path().join("cache"),
        runtime_dir: root.path().join("runtime"),
    };
    let path = root.path().join("provider.json");
    let args = vec!["--config".into(), path.to_string_lossy().into_owned()];
    for config in [
        json!({"images":"files","account_scope":"account"}),
        json!({"images":false}),
        json!({"images":"oops"}),
        json!({"images":"files"}),
    ] {
        fs::write(&path, config.to_string()).unwrap();
        assert!(handle("provider_prepare", input(), &args, &paths, root.path().to_str().unwrap(), None).is_err());
    }
    fs::write(&path, json!({"images":"files","account_scope":"account","expires_after_seconds":10}).to_string())
        .unwrap();
    let mut data = input();
    data["allow_operations"] = json!(true);
    assert!(handle("provider_prepare", data, &args, &paths, root.path().to_str().unwrap(), None).is_err());
    assert!(!paths.data_dir.exists());
    let config = json!({"images":"files","account_scope":"account"});
    fs::write(&path, config.to_string()).unwrap();
    let mut data = input();
    data["allow_operations"] = json!(true);
    data["extra_body"]["stream"] = json!(true);
    assert!(handle("provider_prepare", data.clone(), &args, &paths, root.path().to_str().unwrap(), None).is_err());
    data["extra_body"] = json!({});
    data["history"][0]["content"][0]["image_url"]["detail"] = json!("high");
    assert!(handle("provider_prepare", data, &args, &paths, root.path().to_str().unwrap(), None).is_err());
    assert!(!paths.data_dir.exists());
}
#[test]
fn permanent_uploads_require_explicit_null_and_config_changes_fail_closed() {
    let root = tempfile::tempdir().unwrap();
    let data = input();
    let plan = files(data.clone(), "account", None, root.path()).unwrap();
    let wire = STANDARD.decode(plan["operation"]["body_base64"].as_str().unwrap()).unwrap();
    assert!(!String::from_utf8_lossy(&wire).contains("expires_after"));
    let mut resumed = data;
    resumed["operation_state"] = plan["state"].clone();
    resumed["operation_result"] = json!({"id":"file-api-permanent"});
    assert!(files(resumed, "account", None, root.path()).unwrap().get("operation").is_none());
    let paths = AppPaths {
        config_dir: root.path().join("config"),
        data_dir: root.path().join("data"),
        cache_dir: root.path().join("cache"),
        runtime_dir: root.path().join("runtime"),
    };
    let mut changed = input();
    changed["operation_result"] = json!({"id":"file-api-old"});
    changed["operation_state"] = json!({"config":"old-config-hash"});
    assert!(
        handle("provider_prepare", changed, &[], &paths, root.path().to_str().unwrap(), None)
            .unwrap_err()
            .to_string()
            .contains("configuration changed")
    );
}
