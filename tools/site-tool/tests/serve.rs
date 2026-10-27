use i_site_tool::serve;
use std::fs;
#[test]
fn static_preview_has_safe_paths_and_wasm_mime() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("index.html"), "site").unwrap();
    fs::write(temp.path().join("file.wasm"), b"\0asm").unwrap();
    let asset = serve::asset(temp.path(), "/file.wasm?version=1").unwrap();
    assert_eq!(asset.mime, "application/wasm");
    assert_eq!(asset.bytes, b"\0asm");
    assert_eq!(serve::asset(temp.path(), "/").unwrap().bytes, b"site");
    for target in ["/../private", "/%2e%2e/private", "/%00", "/%zz", "//host/file", "/a\\b", "https://evil.test/file"] {
        assert!(serve::asset(temp.path(), target).is_err(), "{target}");
    }
}
#[cfg(unix)]
#[test]
fn preview_rejects_symlink_escape() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret"), "not public").unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("escape")).unwrap();
    assert!(serve::asset(root.path(), "/escape/secret").is_err());
}
