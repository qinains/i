use super::*;
fn fixture(root: &Path) -> (Value, Value, Value) {
    let data = json!({"protocol":"deepseek","base_url":"https://example.invalid/v1","credential_scope":"key",
        "operations_remaining":8,"action":{"op":"files.gc"}});
    let scope = file_api::scope(&data, &endpoint(&data).unwrap(), "account").unwrap();
    let config = json!({"auto_gc":{"enabled":true}});
    let row = json!({"id":"file-old","scope":scope,"owned":true,"status":"retired","retired_at":1,
        "expires_at":null,"filename":"image.png","bytes":12});
    commit(&root.join("provider-files.json"), &json!({"version":1,"entries":{},"retired":{"old":row}})).unwrap();
    (data, config, row)
}
#[test]
fn report_classifies_every_current_scope_record_without_mutating_or_dispatching() {
    let root = tempfile::tempdir().unwrap();
    let (data, config, row) = fixture(root.path());
    let t = now().unwrap();
    let mut db = json!({"version":1,"entries":{},"retired":{}});
    for reason in [
        "delete_remote",
        "prune_local",
        "wait_expiry",
        "grace_period",
        "referenced",
        "not_owned",
        "unknown_outcome",
        "invalid_metadata",
        "invalid_state",
        "other_scope",
    ] {
        let mut row = row.clone();
        row["id"] = json!(format!("file-{reason}"));
        match reason {
            "prune_local" => row["expires_at"] = json!(1),
            "wait_expiry" => row["expires_at"] = json!(t + 30),
            "grace_period" => row["retired_at"] = json!(t),
            "referenced" => db["entries"]["ref"] = json!({"id":row["id"],"status":"ready"}),
            "not_owned" => row["owned"] = json!(false),
            "unknown_outcome" => row["status"] = json!("deleting"),
            "invalid_metadata" => row["expires_at"] = json!("bad"),
            "invalid_state" => row["status"] = json!("unexpected"),
            "other_scope" => row["scope"] = json!("other"),
            _ => (),
        }
        db["retired"][reason] = row;
    }
    let path = root.path().join("provider-files.json");
    commit(&path, &db).unwrap();
    let before = fs::read(&path).unwrap();
    let out = manage(data, &config, "account", root.path()).unwrap();
    assert!(out.get("operation").is_none());
    assert_eq!(out["result"]["records"].as_array().unwrap().len(), 9);
    for count in out["result"]["counts"].as_object().unwrap().values() {
        assert_eq!(count, 1);
    }
    assert_eq!(out["result"]["cleanup_authorized"], false);
    assert_eq!(fs::read(&path).unwrap(), before);
    let mut bad = row.clone();
    bad["id"] = json!("invalid-id");
    bad["expires_at"] = json!(1);
    let mut broken = json!({"version":1,"entries":{},"retired":{"bad":bad}});
    commit(&path, &broken).unwrap();
    let mut authorized = json!({"protocol":"deepseek","base_url":"https://example.invalid/v1","credential_scope":"key","allow_cleanup":true,"operations_remaining":8});
    assert!(next(&authorized, &config, "account", root.path()).is_err());
    assert_eq!(load(&path).unwrap(), broken);
    bad["status"] = json!("ready");
    broken["entries"]["bad"] = bad;
    authorized = json!({"auto_gc":{"enabled":true},"auto_refresh":{"enabled":true}});
    assert!(!prune_ready(&mut broken, row["scope"].as_str().unwrap(), &authorized, t));
}
#[test]
fn inspection_is_read_only_scoped_and_rejects_changed_or_mismatched_responses() {
    let root = tempfile::tempdir().unwrap();
    let (mut data, config, mut row) = fixture(root.path());
    row["status"] = json!("deleting");
    let path = root.path().join("provider-files.json");
    commit(&path, &json!({"version":1,"entries":{},"retired":{"old":row}})).unwrap();
    let before = fs::read(&path).unwrap();
    data["action"] = json!({"op":"files.gc-inspect","key":"old"});
    let plan = inspect(data.clone(), &config, "account", root.path()).unwrap();
    assert_eq!(plan["operation"]["method"], "GET");
    assert_eq!(plan["operation"]["headers"]["cache-control"], "no-cache");
    let mut reply = data.clone();
    reply["operation_state"] = plan["state"].clone();
    reply["operation_result"] =
        json!({"id":"file-old","object":"file","filename":"image.png","bytes":12,"created_at":1,"purpose":"user_data"});
    let result = inspect(reply.clone(), &config, "account", root.path()).unwrap();
    assert_eq!(result["result"]["metadata_matches_snapshot"], true);
    assert_eq!(result["result"]["local_status"], "deleting");
    assert_eq!(fs::read(&path).unwrap(), before);
    reply["operation_result"]["bytes"] = json!(13);
    assert_eq!(
        inspect(reply.clone(), &config, "account", root.path()).unwrap()["result"]["metadata_matches_snapshot"],
        false
    );
    reply["operation_result"]["id"] = json!("file-other");
    assert!(inspect(reply.clone(), &config, "account", root.path()).is_err());
    assert!(inspect(data, &config, "wrong-account", root.path()).is_err());
    row["nonce"] = json!("changed");
    commit(&path, &json!({"version":1,"entries":{},"retired":{"old":row}})).unwrap();
    reply["operation_result"]["id"] = json!("file-old");
    assert!(inspect(reply, &config, "account", root.path()).is_err());
}
