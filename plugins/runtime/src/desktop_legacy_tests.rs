use super::*;
fn paths(root: &Path) -> AppPaths {
    AppPaths {
        config_dir: root.join("config"),
        data_dir: root.join("data"),
        cache_dir: root.join("cache"),
        runtime_dir: root.join("runtime"),
    }
}
#[test]
fn legacy_generated_search_is_replaced_by_core_tools_and_workers_are_rebound() {
    let root = tempfile::tempdir().unwrap();
    let paths = paths(root.path());
    let old = json!({"tools":[
        {"name":"search","exec":["/retired/i-plugins","search"],"cwd":paths.config_dir},
        {"name":"find_files","exec":["/retired/i-plugins","search"],"cwd":paths.config_dir},
        {"name":"memory_get","exec":["/retired/i-plugins","memory"],"cwd":paths.config_dir}],
        "hooks":[{"events":["session_load"],"exec":["/retired/i-plugins","cache"],"cwd":paths.config_dir}]});
    let upgraded = resolve_legacy(old.clone(), &paths).unwrap();
    assert_eq!(upgraded["tools"].as_array().unwrap().len(), 1);
    assert_eq!(upgraded["tools"][0]["name"], "memory_get");
    assert_eq!(upgraded["tools"][0]["exec"][0], crate::executable().unwrap()[0]);
    assert_eq!(upgraded["hooks"][0]["exec"][0], crate::executable().unwrap()[0]);
    assert_eq!(old["tools"].as_array().unwrap().len(), 3);
    assert!(upgraded.get("analyses").is_none(), "absent optional arrays must not become null");
}
#[test]
fn custom_search_and_custom_workers_are_never_removed_or_rewritten() {
    let root = tempfile::tempdir().unwrap();
    let paths = paths(root.path());
    for (exec, cwd) in [
        (json!(["/custom/worker", "search"]), json!(paths.config_dir)),
        (json!(["/retired/i-plugins", "search"]), json!(root.path())),
    ] {
        let value = json!({"tools":[{"name":"search","exec":exec,"cwd":cwd}]});
        assert_eq!(resolve_legacy(value.clone(), &paths).unwrap(), value);
    }
}
