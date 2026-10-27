use super::*;
fn metadata() -> Value {
    json!({"object":"file","id":"file-api-one","filename":"image-test.png","bytes":12,
        "created_at":1,"purpose":"user_data","expires_at":9999999999u64})
}
pub(super) fn manage(root: &Path, base: &str, action: Value, confirmation: &str) -> Output {
    let mut command = Command::new(binary());
    support::isolate(&mut command, root);
    command.env_remove("API_MODEL").env("FILES_TEST_KEY", "test-key").args([
        "--config",
        root.join("api.json").to_str().unwrap(),
        "-C",
        root.to_str().unwrap(),
        "--base-url",
        base,
        "--yolo",
        "--provider-action",
        &action.to_string(),
    ]);
    let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(confirmation.as_bytes()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() > deadline {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("management timed out");
        }
        thread::sleep(Duration::from_millis(5));
    }
    child.wait_with_output().unwrap()
}
#[test]
fn list_get_and_confirmed_delete_need_no_model_or_session() {
    let root = tempfile::tempdir().unwrap();
    configuration(root.path(), true);
    let listing = json!({"object":"list","data":[metadata()],"has_more":true,"last_id":"file-api-one"});
    let deleted = json!({"id":"file-api-one","object":"file","deleted":true});
    let (base, server) =
        server(vec![Some((200, listing.clone())), Some((200, metadata())), Some((200, deleted.clone()))]);
    for (action, reply) in [
        (json!({"op":"files.list","limit":1}), listing),
        (json!({"op":"files.get","file_id":"file-api-one"}), metadata()),
        (json!({"op":"files.delete","file_id":"file-api-one"}), deleted),
    ] {
        let out = manage(root.path(), &base, action.clone(), "y\n");
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        assert_eq!(serde_json::from_slice::<Value>(&out.stdout).unwrap(), reply);
        if action["op"] == "files.delete" {
            assert!(String::from_utf8_lossy(&out.stderr).contains("REMOTE DELETE"));
        }
    }
    let (requests, listener) = server.join().unwrap();
    assert_eq!(requests.len(), 3);
    assert!(requests[0].0.starts_with("GET /files?purpose=user_data&limit=1&order=asc HTTP/1.1"));
    assert!(requests[1].0.starts_with("GET /files/file-api-one HTTP/1.1"));
    assert!(requests[2].0.starts_with("DELETE /files/file-api-one HTTP/1.1"));
    assert!(requests.iter().all(|(h, b)| b.is_empty() && h.contains("Bearer test-key")));
    assert!(!root.path().join("session.json").exists());
    assert_eq!(listener.accept().unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
}
#[test]
fn delete_denial_and_disabled_management_never_send_http() {
    for enabled in [true, false] {
        let root = tempfile::tempdir().unwrap();
        configuration(root.path(), enabled);
        let (base, server) = server(vec![]);
        let output = manage(root.path(), &base, json!({"op":"files.delete","file_id":"file-api-one"}), "n\n");
        assert!(!output.status.success());
        if enabled {
            assert!(String::from_utf8_lossy(&output.stderr).contains("user denied action"));
        }
        let (_, listener) = server.join().unwrap();
        assert_eq!(listener.accept().unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
    }
}
#[test]
fn deletion_failure_has_no_network_retry_or_model_fallback() {
    for reply in [
        None,
        Some((503, json!({"error":"unknown"}))),
        Some((200, json!({"id":"file-api-one","object":"file","deleted":false}))),
    ] {
        let root = tempfile::tempdir().unwrap();
        configuration(root.path(), true);
        let (base, server) = server(vec![reply]);
        let output = manage(root.path(), &base, json!({"op":"files.delete","file_id":"file-api-one"}), "y\n");
        assert!(!output.status.success());
        let (requests, listener) = server.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(listener.accept().unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
    }
}
#[test]
fn custom_provider_cannot_delete_from_a_model_turn_or_escape_origin() {
    for model in [None, Some("optional-model-hint")] {
        let root = tempfile::tempdir().unwrap();
        let path = binding(root.path(), json!({"kind":"model"}));
        let mut config: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        if let Some(model) = model {
            config["model"] = json!(model);
        }
        fs::write(&path, config.to_string()).unwrap();
        let (base, server) = server(vec![]);
        let out = manage(root.path(), &base, json!({"op":"custom"}), "");
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        assert_eq!(serde_json::from_slice::<Value>(&out.stdout).unwrap(), json!(model.unwrap_or_default()));
        assert_eq!(server.join().unwrap().1.accept().unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
    }
    for mode in ["model", "cleanup", "origin", "method", "body"] {
        let root = tempfile::tempdir().unwrap();
        let config = binding(root.path(), json!({"kind":"delete","mode":mode}));
        let mut value: Value = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
        value["provider_operations"] = json!(true);
        fs::write(&config, value.to_string()).unwrap();
        let (base, server) = server(vec![]);
        let output = if matches!(mode, "model" | "cleanup") {
            let mut command = Command::new(binary());
            support::isolate(&mut command, root.path());
            command.args([
                "--config",
                config.to_str().unwrap(),
                "-C",
                root.path().to_str().unwrap(),
                "--base-url",
                &base,
                "--model",
                "fixture",
                "hello",
            ]);
            bounded(&mut command)
        } else {
            manage(root.path(), &base, json!({"op":"custom"}), "y\n")
        };
        assert!(!output.status.success(), "{mode}");
        let (_, listener) = server.join().unwrap();
        assert_eq!(listener.accept().unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
    }
}
