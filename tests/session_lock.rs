#![cfg(feature = "native")]

use std::{
    fs::{self, OpenOptions},
    process::Command,
};

#[test]
fn another_process_cannot_run_or_delete_a_locked_session() {
    let root = tempfile::tempdir().unwrap();
    let session = root.path().join("conversation.json");
    fs::write(&session, "retained checkpoint").unwrap();
    fs::write(root.path().join("config.json"), "{}").unwrap();
    let locks = root.path().join(".i-session-locks");
    fs::create_dir(&locks).unwrap();
    let lock_path = locks.join("conversation.json");
    let owner = OpenOptions::new().read(true).write(true).create_new(true).open(&lock_path).unwrap();
    owner.try_lock().unwrap();

    let invoke = |extra: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_i"))
            .current_dir(root.path())
            .args(["--session", "./conversation.json", "--config", "./config.json"])
            .args(extra)
            .output()
            .unwrap()
    };
    for extra in [
        &["--base-url", "http://127.0.0.1:1/v1", "--model", "fixture", "do not send"][..],
        &["--sessions", "delete", "--yolo"][..],
    ] {
        let result = invoke(extra);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("session is in use"), "{:?}", result);
        assert_eq!(fs::read_to_string(&session).unwrap(), "retained checkpoint");
    }
    drop(owner);
    let deleted = invoke(&["--sessions", "delete", "--yolo"]);
    assert!(deleted.status.success(), "{:?}", deleted);
    assert!(!session.exists());
    assert!(lock_path.exists());
}
