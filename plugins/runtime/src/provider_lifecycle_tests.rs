use super::*;
#[test]
fn responses_and_anthropic_multipart_auth_and_native_image_references() {
    for kind in ["responses", "anthropic"] {
        let root = tempfile::tempdir().unwrap();
        let mut data = input();
        data["protocol"] = json!(kind);
        let ttl = if kind == "anthropic" { None } else { Some(3600) };
        let plan = files(data.clone(), "account", ttl, root.path()).unwrap();
        assert_eq!(plan["operation"]["url"], "https://example.invalid/v1/files");
        let bytes = STANDARD.decode(plan["operation"]["body_base64"].as_str().unwrap()).unwrap();
        let body = String::from_utf8_lossy(&bytes);
        assert!(body.contains("image/png"));
        let id = if kind == "anthropic" { "file_fixture" } else { "file-fixture" };
        if kind == "anthropic" {
            assert!(!body.contains("name=\"purpose\"") && !body.contains("expires_after"));
            assert_eq!(plan["operation"]["auth"]["header"], "x-api-key");
            assert_eq!(plan["operation"]["headers"]["anthropic-beta"], "files-api-2025-04-14");
        } else {
            assert!(body.contains("name=\"purpose\"\r\n\r\nvision"));
        }
        let mut reply = data.clone();
        reply["operation_state"] = plan["state"].clone();
        reply["operation_result"] = json!({"id":id,"expires_at":ttl.map(|t|now().unwrap()+t)});
        let prepared = files(reply, "account", ttl, root.path()).unwrap();
        if kind == "anthropic" {
            assert_eq!(
                prepared["body"]["messages"][0]["content"][0],
                json!({"type":"image","source":{"type":"file","file_id":id}})
            );
            assert_eq!(prepared["headers"]["anthropic-beta"], "files-api-2025-04-14");
        } else {
            assert_eq!(prepared["body"]["input"][0]["content"][0], json!({"type":"input_image","file_id":id}));
        }
        assert_eq!(files(data, "account", ttl, root.path()).unwrap(), prepared);
    }
}
#[test]
fn refresh_is_opt_in_archives_old_id_and_never_replays_pending_upload() {
    let root = tempfile::tempdir().unwrap();
    let mut data = input();
    let original = files(data.clone(), "account", Some(3600), root.path()).unwrap();
    finish(root.path(), &data, &original);
    let path = root.path().join("provider-files.json");
    let mut db: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let key = original["state"]["key"].as_str().unwrap();
    db["entries"][key]["expires_at"] = json!(now().unwrap() + 10);
    commit(&path, &db).unwrap();
    assert!(files(data.clone(), "account", Some(3600), root.path()).is_err());
    data["_files_policy"] = json!({"auto_refresh":{"enabled":true}});
    let mut damaged = db.clone();
    damaged["entries"][key]["id"] = json!("invalid-id");
    commit(&path, &damaged).unwrap();
    let before = fs::read(&path).unwrap();
    assert!(files(data.clone(), "account", Some(3600), root.path()).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    commit(&path, &db).unwrap();
    let refresh = files(data.clone(), "account", Some(3600), root.path()).unwrap();
    assert!(refresh.get("operation").is_some());
    let saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["retired"].as_object().unwrap().len(), 1);
    assert_eq!(saved["retired"].as_object().unwrap().values().next().unwrap()["owned"], true);
    assert!(
        files(data.clone(), "account", Some(3600), root.path()).unwrap_err().to_string().contains("pending/unknown")
    );
    let mut reply = data.clone();
    reply["operation_state"] = refresh["state"].clone();
    reply["operation_result"] = json!({"id":"file-new","expires_at":now().unwrap()+3600});
    assert_eq!(
        files(reply, "account", Some(3600), root.path()).unwrap()["body"]["messages"][0]["content"][0]["file_id"],
        "file-new"
    );
    assert!(files(data, "account", Some(3600), root.path()).unwrap().get("operation").is_none());
}
#[test]
fn gc_only_collects_owned_superseded_unreferenced_ids_and_never_replays_unknown_delete() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("provider-files.json");
    let mut data = input();
    data["allow_cleanup"] = json!(true);
    data["operations_remaining"] = json!(8);
    let scope = file_api::scope(&data, &endpoint(&data).unwrap(), "account").unwrap();
    let config = json!({"auto_gc":{"enabled":true}});
    let row = json!({"id":"file-old","scope":scope,"status":"retired","owned":true,"retired_at":now().unwrap()-7200,"expires_at":null});
    for change in ["imported", "scope", "active", "unknown", "grace"] {
        let mut db = json!({"version":1,"entries":{},"retired":{"old":row}});
        match change {
            "imported" => db["retired"]["old"]["owned"] = json!(false),
            "scope" => db["retired"]["old"]["scope"] = json!("other"),
            "active" => db["entries"]["new"] = json!({"id":"file-old","status":"ready"}),
            "unknown" => db["retired"]["old"]["status"] = json!("deleting"),
            _ => db["retired"]["old"]["retired_at"] = json!(now().unwrap()),
        }
        commit(&path, &db).unwrap();
        assert!(lifecycle::next(&data, &config, "account", root.path()).unwrap().is_none(), "{change}");
    }
    commit(&path, &json!({"version":1,"entries":{},"retired":{"old":row}})).unwrap();
    let mut forbidden = data.clone();
    forbidden["allow_cleanup"] = json!(false);
    assert!(lifecycle::next(&forbidden, &config, "account", root.path()).is_err());
    let plan = lifecycle::next(&data, &config, "account", root.path()).unwrap().unwrap();
    assert_eq!(plan["operation"]["cleanup"], true);
    assert_eq!(plan["operation"]["method"], "DELETE");
    assert!(lifecycle::next(&data, &config, "account", root.path()).unwrap().is_none());
    let mut reply = data.clone();
    reply["operation_state"] = plan["state"].clone();
    reply["operation_result"] = json!({"id":"file-wrong","object":"file","deleted":true});
    assert!(lifecycle::finish(&mut reply, "account", root.path()).is_err());
    reply["operation_result"]["id"] = json!("file-old");
    lifecycle::finish(&mut reply, "account", root.path()).unwrap();
    assert!(
        serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap()["retired"].as_object().unwrap().is_empty()
    );
    let mut both = config.clone();
    both["auto_refresh"] = json!({"enabled":true});
    let mut pruning = json!({"entries":{"expired":{"id":"file-expired","scope":scope,"status":"ready","owned":true,"expires_at":1},
        "unknown":{"scope":scope,"status":"pending","owned":true,"expires_at":1},
        "imported":{"scope":scope,"status":"ready","owned":false,"expires_at":1}}});
    assert!(lifecycle::prune_ready(&mut pruning, &scope, &both, now().unwrap()));
    assert!(pruning["entries"].get("expired").is_none());
    assert_eq!(pruning["entries"].as_object().unwrap().len(), 2);
    let mut expired = row;
    expired["expires_at"] = json!(1);
    commit(&path, &json!({"version":1,"entries":{},"retired":{"old":expired}})).unwrap();
    assert!(lifecycle::next(&data, &config, "account", root.path()).unwrap().is_none());
    assert!(
        serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap()["retired"].as_object().unwrap().is_empty()
    );
}
#[test]
fn lifecycle_policy_and_anthropic_management_wire_are_strict() {
    for policy in [
        json!({"auto_gc":true}),
        json!({"auto_refresh":{"enabled":true,"max_age_seconds":1}}),
        json!({"auto_gc":{"enabled":true,"max_age_seconds":3600}}),
        json!({"auto_gc":{"enabled":"yes"}}),
    ] {
        assert!(file_api::validate(&policy).is_err());
    }
    let data = json!({"protocol":"anthropic"});
    let meta = json!({"id":"file_example","type":"file","filename":"image.png","size_bytes":12,"created_at":"2026-01-01T00:00:00Z"});
    assert_eq!(file_api::metadata(&data, &meta).unwrap()["bytes"], 12);
    assert!(file_api::deleted(&data, &json!({"id":"file_example","type":"file_deleted"}), "file_example").is_ok());
    assert!(file_api::deleted(&data, &json!({"id":"file_example","deleted":true}), "file_example").is_err());
}
