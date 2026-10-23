use super::*;
#[test]
fn openai_responses_and_anthropic_files_use_real_multipart_and_resume() {
    for kind in ["responses", "anthropic"] {
        let root = tempfile::tempdir().unwrap();
        let api = configuration(root.path(), true);
        let mut config: Value = serde_json::from_slice(&fs::read(&api).unwrap()).unwrap();
        config["protocol"] = json!(kind);
        config["extra_body"] = json!({});
        fs::write(api, config.to_string()).unwrap();
        let id = if kind == "anthropic" { "file_example" } else { "file-example" };
        if kind == "anthropic" {
            fs::write(
                root.path().join(".app/config/provider.json"),
                json!({"images":"files","account_scope":"fixture","expires_after_seconds":null}).to_string(),
            )
            .unwrap();
        }
        let upload = if kind == "anthropic" { json!({"id":id,"type":"file"}) } else { uploaded(id) };
        let answer = if kind == "anthropic" {
            json!({"type":"message","role":"assistant","stop_reason":"end_turn","content":[{"type":"text","text":"image answer"}]})
        } else {
            json!({"status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"image answer"}]}]})
        };
        let meta = if kind == "anthropic" {
            json!({"id":id,"type":"file","filename":"image.png","size_bytes":12,"created_at":"2026-01-01T00:00:00Z"})
        } else {
            json!({"id":id,"object":"file","filename":"image.png","bytes":12,"created_at":1,"purpose":"vision"})
        };
        let listing = json!({"object":"list","data":[meta.clone()],"has_more":false,"last_id":id});
        let deleted = if kind == "anthropic" {
            json!({"id":id,"type":"file_deleted"})
        } else {
            json!({"id":id,"object":"file","deleted":true})
        };
        let (base, server) = server(vec![
            Some((200, upload)),
            Some((200, answer.clone())),
            Some((200, answer)),
            Some((200, listing.clone())),
            Some((200, meta.clone())),
            Some((200, deleted.clone())),
        ]);
        for first in [true, false] {
            let out = invoke(root.path(), &base, "test-key", first);
            assert!(out.status.success(), "{kind}: {}", String::from_utf8_lossy(&out.stderr));
            if first {
                fs::remove_file(root.path().join("image.png")).unwrap();
            }
        }
        for (action, result) in [
            (json!({"op":"files.list"}), listing),
            (json!({"op":"files.get","file_id":id}), meta),
            (json!({"op":"files.delete","file_id":id}), deleted),
        ] {
            let out = super::management::manage(root.path(), &base, action, "y\n");
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            assert_eq!(serde_json::from_slice::<Value>(&out.stdout).unwrap(), result);
        }
        let (requests, listener) = server.join().unwrap();
        assert_eq!(requests.len(), 6);
        assert!(requests[5].0.starts_with(&format!("DELETE /v1/files/{id} ")));
        assert!(requests[0].0.starts_with("POST /v1/files "));
        let multipart = String::from_utf8_lossy(&requests[0].1);
        assert_eq!(multipart.contains("name=\"purpose\"\r\n\r\nvision"), kind == "responses");
        let body: Value = serde_json::from_slice(&requests[1].1).unwrap();
        if kind == "anthropic" {
            assert!(requests[0].0.to_lowercase().contains("x-api-key: test-key"));
            assert!(requests[1].0.to_lowercase().contains("anthropic-beta: files-api-2025-04-14"));
            assert!(body["messages"].to_string().contains("\"file_id\":\"file_example\""));
        } else {
            assert!(body["input"].to_string().contains("\"file_id\":\"file-example\""));
        }
        let session = fs::read_to_string(root.path().join("session.json")).unwrap();
        assert!(session.contains("data:image/png;base64,"));
        assert_eq!(listener.accept().unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
    }
}
#[test]
fn automatic_refresh_and_scoped_cleanup_require_separate_grants_and_preserve_current_id() {
    for lost in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let api = configuration(root.path(), true);
        let (base, server) = server(vec![
            Some((200, uploaded("file-first"))),
            Some((200, answer())),
            Some((200, uploaded("file-second"))),
            Some((200, answer())),
            if lost { None } else { Some((200, json!({"id":"file-first","object":"file","deleted":true}))) },
            Some((200, answer())),
        ]);
        assert!(invoke(root.path(), &base, "test-key", true).status.success());
        let store = support::store(root.path()).join("provider-files.json");
        let mut db: Value = serde_json::from_slice(&fs::read(&store).unwrap()).unwrap();
        let row = db["entries"].as_object_mut().unwrap().values_mut().next().unwrap();
        row["created_at"] = json!(1);
        row["expires_at"] = Value::Null;
        fs::write(&store, db.to_string()).unwrap();
        let policy = json!({"images":"files","account_scope":"fixture","expires_after_seconds":3600,
        "auto_refresh":{"enabled":true,"max_age_seconds":3600},"auto_gc":{"enabled":true}});
        fs::write(root.path().join(".app/config/provider.json"), policy.to_string()).unwrap();
        let denied = invoke(root.path(), &base, "test-key", false);
        assert!(!denied.status.success());
        assert!(String::from_utf8_lossy(&denied.stderr).contains("provider_cleanup"));
        let mut config: Value = serde_json::from_slice(&fs::read(&api).unwrap()).unwrap();
        config["provider_cleanup"] = json!(true);
        fs::write(api, config.to_string()).unwrap();
        let out = invoke(root.path(), &base, "test-key", false);
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let mut db: Value = serde_json::from_slice(&fs::read(&store).unwrap()).unwrap();
        db["retired"].as_object_mut().unwrap().values_mut().next().unwrap()["retired_at"] = json!(1);
        fs::write(&store, db.to_string()).unwrap();
        let out = invoke(root.path(), &base, "test-key", false);
        if lost {
            assert!(!out.status.success());
            assert!(invoke(root.path(), &base, "test-key", false).status.success());
        } else {
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        }
        let db: Value = serde_json::from_slice(&fs::read(&store).unwrap()).unwrap();
        if lost {
            assert_eq!(db["retired"].as_object().unwrap().values().next().unwrap()["status"], "deleting");
        } else {
            assert!(db["retired"].as_object().unwrap().is_empty());
        }
        assert_eq!(db["entries"].as_object().unwrap().values().next().unwrap()["id"], "file-second");
        let (requests, listener) = server.join().unwrap();
        assert_eq!(requests.len(), 6);
        assert!(requests[4].0.starts_with("DELETE /files/file-first "));
        assert!(
            serde_json::from_slice::<Value>(&requests[5].1).unwrap()["messages"].to_string().contains("file-second")
        );
        assert_eq!(listener.accept().unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
    }
}
