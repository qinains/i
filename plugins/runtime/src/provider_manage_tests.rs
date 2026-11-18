use super::*;
fn setup(root: &Path) -> (AppPaths, Vec<String>, Value) {
    let paths = AppPaths {
        config_dir: root.join("config"),
        data_dir: root.join("data"),
        cache_dir: root.join("cache"),
        runtime_dir: root.join("runtime"),
    };
    let config = root.join("provider.json");
    fs::write(&config, json!({"images":"base64","account_scope":"acct"}).to_string()).unwrap();
    let args = vec!["--config".into(), config.to_string_lossy().into_owned()];
    let data = json!({"version":1,"protocol":"deepseek","base_url":"https://example.invalid/v1", "credential_scope":"key-fingerprint",
        "allow_operations":true,"operations_remaining":8,"action":{"op":"files.list"}});
    (paths, args, data)
}
fn file(id: &str) -> Value {
    json!({"id":id,"object":"file","filename":"image-fixture.png","bytes":123,"created_at":1,
        "purpose":"user_data","expires_at":now().unwrap()+1000})
}
fn call(data: Value, args: &[String], paths: &AppPaths, root: &Path) -> Result<Value> {
    handle(data, args, paths, root.to_str().unwrap(), None)
}
fn reply(mut data: Value, plan: &Value, result: Value) -> Value {
    data["operation_state"] = plan["state"].clone();
    data["operation_result"] = result;
    data
}
#[test]
fn list_query_pagination_and_argument_validation() {
    let root = tempfile::tempdir().unwrap();
    let (paths, args, mut data) = setup(root.path());
    data["action"] = json!({"op":"files.list","limit":2,"after":"file-api-before","order":"desc"});
    let plan = call(data.clone(), &args, &paths, root.path()).unwrap();
    assert_eq!(plan["operation"]["method"], "GET");
    assert_eq!(plan["operation"]["body_base64"], "");
    let url = url::Url::parse(plan["operation"]["url"].as_str().unwrap()).unwrap();
    let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(query["purpose"], "user_data");
    assert_eq!(query["after"], "file-api-before");
    assert_eq!(query["limit"], "2");
    let result = json!({"object":"list","data":[file("file-api-one")],"has_more":true,"last_id":"file-api-one"});
    assert_eq!(call(reply(data.clone(), &plan, result.clone()), &args, &paths, root.path()).unwrap()["result"], result);
    let mut wrong = result;
    wrong["last_id"] = json!("wrong");
    assert!(call(reply(data.clone(), &plan, wrong), &args, &paths, root.path()).is_err());
    for action in [
        json!({"op":"files.list","limit":0}),
        json!({"op":"files.list","limit":1001}),
        json!({"op":"files.list","after":"../secret"}),
        json!({"op":"files.list","order":"desc&x=y"}),
        json!({"op":"files.get","file_id":"file-api-a/../../other"}),
        json!({"op":"files.delete","file_id":"file-api-a","recursive":true}),
    ] {
        data["action"] = action;
        assert!(call(data.clone(), &args, &paths, root.path()).is_err());
    }
}
#[test]
fn reconcile_then_delete_preserves_tombstone_and_refuses_mismatched_metadata() {
    let root = tempfile::tempdir().unwrap();
    let (paths, args, mut data) = setup(root.path());
    let store = paths.workspace(root.path(), None).unwrap().1;
    fs::create_dir_all(&store).unwrap();
    let path = store.join("provider-files.json");
    let scope = digest(&json!(["https://example.invalid/files", "acct", "key-fingerprint"]));
    commit(&path,&json!({"version":1,"entries":{"key":{"scope":scope,"status":"pending","filename":"image-fixture.png","bytes":123,"ttl":3600}}})).unwrap();
    data["action"] = json!({"op":"files.reconcile","key":"key","file_id":"file-api-one"});
    assert!(call(data.clone(), &args, &paths, root.path()).is_err());
    data["action"]["confirm"] = json!(true);
    let plan = call(data.clone(), &args, &paths, root.path()).unwrap();
    let mut wrong = file("file-api-one");
    wrong["bytes"] = json!(124);
    assert!(call(reply(data.clone(), &plan, wrong), &args, &paths, root.path()).is_err());
    assert!(call(reply(data.clone(), &plan, file("file-api-other")), &args, &paths, root.path()).is_err());
    call(reply(data.clone(), &plan, file("file-api-one")), &args, &paths, root.path()).unwrap();
    let read = || serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(read()["entries"]["key"]["status"], "ready");
    data["action"] = json!({"op":"files.delete","file_id":"file-api-one"});
    let plan = call(data.clone(), &args, &paths, root.path()).unwrap();
    assert_eq!(plan["operation"]["method"], "DELETE");
    assert_eq!(read()["entries"]["key"]["status"], "deleting");
    assert!(
        call(
            reply(data.clone(), &plan, json!({"object":"file","id":"file-api-one","deleted":false})),
            &args,
            &paths,
            root.path()
        )
        .is_err()
    );
    assert_eq!(read()["entries"]["key"]["status"], "deleting");
    call(
        reply(data.clone(), &plan, json!({"object":"file","id":"file-api-one","deleted":true})),
        &args,
        &paths,
        root.path(),
    )
    .unwrap();
    assert_eq!(read()["entries"]["key"]["status"], "deleted");
    assert!(call(data, &args, &paths, root.path()).is_err());
    let bytes = b"\x89PNG\r\n\x1a\nfixture";
    let image_key = digest(&json!([scope, format!("{:x}", Sha256::digest(bytes))]));
    let mut saved = read();
    saved["entries"][&image_key] = saved["entries"]["key"].clone();
    commit(&path, &saved).unwrap();
    let mut image_data = setup(root.path()).2;
    image_data["history"] = json!([{"role":"user","content":[{"type":"image_url","image_url":{
        "url":format!("data:image/png;base64,{}",STANDARD.encode(bytes))}}]}]);
    assert!(files(image_data, "acct", Some(3600), &store).unwrap_err().to_string().contains("reference deleted"));
    let mut reset = json!({"version":1,"entries":{}});
    reset["entries"] = read()["entries"].clone();
    reset["entries"]["key"]["status"] = json!("deleting");
    commit(&path, &reset).unwrap();
    let mut data = setup(root.path()).2;
    data["action"] = json!({"op":"files.reconcile","key":"key","file_id":"file-api-one","confirm":true});
    let plan = call(data.clone(), &args, &paths, root.path()).unwrap();
    let mut changed = read();
    changed["entries"]["key"]["delete_nonce"] = json!("concurrent-change");
    commit(&path, &changed).unwrap();
    assert!(call(reply(data.clone(), &plan, file("file-api-one")), &args, &paths, root.path()).is_err());
    reset["retired"] = json!({"old":{"id":"file-api-one","status":"deleting"}});
    commit(&path, &reset).unwrap();
    assert!(call(data.clone(), &args, &paths, root.path()).is_err());
    reset["retired"]["old"]["status"] = json!("retired");
    commit(&path, &reset).unwrap();
    let plan = call(data.clone(), &args, &paths, root.path()).unwrap();
    reset["retired"] = json!({});
    commit(&path, &reset).unwrap();
    assert!(call(reply(data, &plan, file("file-api-one")), &args, &paths, root.path()).is_err());
}
