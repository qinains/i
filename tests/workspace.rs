use i::{
    platform::{Capabilities, FileSystem, native::NativeFs},
    workspace::{Limits, Workspace, path::validate, tool_schemas},
};
use serde_json::json;
use std::path::Path;

#[test]
fn reject_workspace_escape() {
    for p in ["../x", "/x", "C:/x", "a\\b", "x\0", "a/../b", ""] {
        assert!(validate(p, false).is_err(), "{p:?}");
    }
    assert!(validate("中文/文件.txt", false).is_ok());
}
#[tokio::test]
async fn unicode_binary_and_edit_semantics() {
    let root = tempfile::tempdir().unwrap();
    let ws = Workspace::new(NativeFs::new(root.path().into()).unwrap(), Limits::browser());
    ws.fs.write_file(Path::new("nested/中文.bin"), &[0, 255]).await.unwrap();
    assert_eq!(ws.fs.read_file(Path::new("nested/中文.bin")).await.unwrap(), [0, 255]);
    ws.execute("write", &json!({"path":"a.txt","content":"same\nsame\n"})).await.unwrap();
    assert!(ws.execute("edit", &json!({"path":"a.txt","edits":[{"old":"same","new":"x"}]})).await.is_err());
    ws.execute("edit", &json!({"path":"a.txt","edits":[{"old":"same","new":"x","line_number":2}]})).await.unwrap();
    assert_eq!(std::fs::read_to_string(root.path().join("a.txt")).unwrap(), "same\nx\n");
    assert!(
        ws.execute("read", &json!({"files":[{"path":"nested/中文.bin"}]})).await.unwrap()["output"]
            .as_str()
            .unwrap()
            .contains("expected UTF-8")
    );
    assert!(ws.execute("shell", &json!({"command":"true"})).await.is_err());
    let listing = ws.execute("list", &json!({})).await.unwrap();
    assert!(listing["output"].as_str().unwrap().contains("a.txt"));
}
#[test]
fn browser_schema_has_files_but_no_shell() {
    let schemas = tool_schemas(Capabilities::BROWSER);
    let names: Vec<_> = schemas.as_array().unwrap().iter().map(|s| s["function"]["name"].as_str().unwrap()).collect();
    for name in ["read", "write", "edit", "view_image", "list", "download", "glob", "search"] {
        assert!(names.contains(&name));
    }
    assert!(!names.contains(&"shell"));
}
#[cfg(unix)]
#[tokio::test]
async fn native_symlink_escape() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("link")).unwrap();
    let fs = NativeFs::new(root.path().into()).unwrap();
    assert!(fs.write_file(Path::new("link/escape"), b"bad").await.is_err());
    assert!(!outside.path().join("escape").exists());
}
#[tokio::test]
async fn file_limit_prevents_write() {
    let root = tempfile::tempdir().unwrap();
    let ws = Workspace::new(
        NativeFs::new(root.path().into()).unwrap(),
        Limits { file_bytes: 2, write_bytes: 2, output_bytes: 100 },
    );
    assert!(ws.execute("write", &json!({"path":"x","content":"abc"})).await.is_err());
    assert!(!root.path().join("x").exists());
}
