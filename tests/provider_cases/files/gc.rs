use super::*;
#[test]
fn gc_report_and_unknown_cleanup_inspection_do_not_reset_or_repeat_deletion() {
    let root = tempfile::tempdir().unwrap();
    configuration(root.path(), true);
    let meta = json!({"id":"file-first","object":"file","filename":"different.png","bytes":1,"created_at":1,"purpose":"user_data"});
    let (base, server) = server(vec![
        Some((200, uploaded("file-first"))),
        Some((200, answer())),
        Some((200, meta)),
        Some((404, json!({"error":"not found"}))),
    ]);
    assert!(invoke(root.path(), &base, "test-key", true).status.success());
    let path = support::store(root.path()).join("provider-files.json");
    let mut db: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let mut row = db["entries"].as_object().unwrap().values().next().unwrap().clone();
    row["status"] = json!("deleting");
    db["retired"] = json!({"old":row});
    fs::write(&path, db.to_string()).unwrap();
    let before = fs::read(&path).unwrap();
    let report = management::manage(root.path(), &base, json!({"op":"files.gc"}), "");
    assert!(report.status.success(), "{}", String::from_utf8_lossy(&report.stderr));
    let report: Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(report["counts"]["unknown_outcome"], 1);
    let action = json!({"op":"files.gc-inspect","key":"old"});
    let inspected = management::manage(root.path(), &base, action.clone(), "");
    assert!(inspected.status.success(), "{}", String::from_utf8_lossy(&inspected.stderr));
    let inspected: Value = serde_json::from_slice(&inspected.stdout).unwrap();
    assert_eq!(inspected["local_status"], "deleting");
    assert_eq!(inspected["metadata_matches_snapshot"], false);
    let missing = management::manage(root.path(), &base, action, "");
    assert!(!missing.status.success());
    assert_eq!(fs::read(&path).unwrap(), before);
    let (requests, listener) = server.join().unwrap();
    assert_eq!(requests.len(), 4);
    for (headers, body) in &requests[2..] {
        assert!(headers.starts_with("GET /files/file-first "));
        assert!(body.is_empty());
    }
    assert_eq!(listener.accept().unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
}
