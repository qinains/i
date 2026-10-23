use i::{
    agent::history::checked_history,
    browser::session::{browser_path, workspace_id},
};
use serde_json::json;
#[test]
fn workspace_ids_are_not_paths() {
    for value in ["default", "12345678-1234-4123-8123-123456789abc"] {
        workspace_id(value).unwrap();
    }
    for value in ["../default", "/tmp", "workspace", "12345678-1234-1123-8123-123456789abc"] {
        assert!(workspace_id(value).is_err());
    }
}
#[test]
fn browser_mounts_do_not_relax_native_paths() {
    assert_eq!(browser_path("/tmp/a", false).unwrap().to_str().unwrap(), "/tmp/a");
    assert_eq!(browser_path("/tmp", false).unwrap().to_str().unwrap(), "/tmp/");
    assert_eq!(browser_path("/workspace/a", false).unwrap().to_str().unwrap(), "a");
    assert_eq!(browser_path("tmp/a", false).unwrap().to_str().unwrap(), "tmp/a");
    assert_eq!(browser_path("/workspace/nested/a", false).unwrap().to_str().unwrap(), "nested/a");
    assert!(browser_path("/tmp/../a", false).is_err());
    assert!(browser_path("/etc/a", false).is_err());
    assert!(i::workspace::path::validate("/tmp/a", false).is_err());
    workspace_id("temp-12345678-1234-4123-8123-123456789abc").unwrap();
}
#[test]
fn imported_history_requires_complete_pairing() {
    assert!(checked_history(json!([{"role":"system","content":"s"},{"role":"tool","tool_call_id":"orphan"}])).is_err());
}
