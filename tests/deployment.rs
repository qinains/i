#![cfg(feature = "bundled")]
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};
fn command(root: &Path) -> Command {
    let mut cmd =
        Command::new(std::env::var_os("I_DEPLOYMENT_TEST_BINARY").unwrap_or_else(|| env!("CARGO_BIN_EXE_i").into()));
    for name in ["CONFIG", "DATA", "CACHE", "RUNTIME"] {
        cmd.env(format!("I_{name}_DIR"), root.join(name.to_lowercase()));
    }
    cmd.env_remove("API_MODEL").env_remove("API_BASE_URL").stdin(Stdio::null());
    cmd
}
fn setup(root: &Path) -> Command {
    let mut cmd = command(root);
    cmd.args(["setup", "--provider", "openai", "--model", "offline-model", "--key-env", "I_SETUP_TEST_KEY"]);
    cmd
}
fn value(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
#[test]
fn simple_setup_is_offline_portable_and_preserves_advanced_configuration() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("config");
    let out = setup(root.path()).env("I_SETUP_TEST_KEY", "do-not-print-the-key").output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(!String::from_utf8_lossy(&out.stderr).contains("do-not-print-the-key"));
    let path = dir.join("config.json");
    let mut cfg = value(&path);
    assert_eq!(cfg["protocol"], "responses");
    assert_eq!(cfg["base_url"], "https://api.openai.com/v1");
    assert_eq!(cfg["plugins"], "builtin.json");
    assert_eq!(cfg["budget"]["enabled"], false);
    assert_eq!(value(&dir.join("builtin.json")), json!({"builtin_profile":"configured","tools":[]}));
    assert!(!dir.join("desktop.json").exists());
    assert!(!cfg.to_string().contains("do-not-print-the-key"));
    cfg["extra_body"] = json!({"temperature":0.25});
    cfg["budget"] = json!({"context_window":32768});
    cfg["max_tokens"] = json!(2048);
    fs::write(&path, cfg.to_string()).unwrap();
    let before = fs::read(&path).unwrap();
    assert!(!setup(root.path()).output().unwrap().status.success());
    assert_eq!(fs::read(&path).unwrap(), before);
    let out = setup(root.path()).arg("--confirm").output().unwrap();
    assert!(out.status.success());
    let after = value(&path);
    assert_eq!(after["extra_body"], cfg["extra_body"]);
    assert_eq!(after["budget"], cfg["budget"]);
    assert_eq!(after["max_tokens"], 2048);
    let saved = fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .find(|e| e.file_name().to_string_lossy().starts_with("config.json.backup-"))
        .unwrap();
    assert_eq!(fs::read(saved.path()).unwrap(), before);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    }
}
#[test]
fn setup_validates_before_saving_and_never_logs_key_input() {
    for args in [
        vec!["--provider", "unknown"],
        vec!["--provider", "deepseek", "--model", ""],
        vec!["--provider", "openai", "--model", "x", "--url", "https://user:password@example.com"],
        vec!["--provider", "openai", "--model", "x", "--key-env", "bad-name"],
    ] {
        let root = tempfile::tempdir().unwrap();
        assert!(!command(root.path()).arg("setup").args(args).output().unwrap().status.success());
        assert!(!root.path().join("config/config.json").exists());
    }
    let root = tempfile::tempdir().unwrap();
    let mut child = command(root.path())
        .args(["setup", "--provider", "deepseek", "--model", "offline", "--key-stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"secret-prefix\ninvalid-control").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(!out.status.success());
    assert!(!String::from_utf8_lossy(&out.stderr).contains("secret-prefix"));
    assert!(!root.path().join("config/config.json").exists());
}
#[test]
fn installed_binary_and_portable_bindings_survive_relocation() {
    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("installed/bin");
    let out = command(root.path()).args(["install", "--bin-dir"]).arg(&bin).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let installed = bin.join(format!("i{}", std::env::consts::EXE_SUFFIX));
    let moved = root.path().join("relocated");
    fs::rename(bin.parent().unwrap(), &moved).unwrap();
    let executable = moved.join("bin").join(installed.file_name().unwrap());
    let mut cmd = Command::new(&executable);
    for name in ["CONFIG", "DATA", "CACHE", "RUNTIME"] {
        cmd.env(format!("I_{name}_DIR"), root.path().join(name.to_lowercase()));
    }
    let out = cmd.args(["--runtime", "manifest"]).output().unwrap();
    assert!(out.status.success());
    let manifest: Value = serde_json::from_slice(&out.stdout).unwrap();
    for section in ["tools", "hooks"] {
        for entry in manifest[section].as_array().unwrap() {
            assert_eq!(entry["exec"][0], executable.to_string_lossy().as_ref());
            assert_eq!(entry["exec"][1], "--runtime");
        }
    }
    assert_eq!(manifest["provider"]["exec"][0], executable.to_string_lossy().as_ref());
    assert!(!root.path().join("config/config.json").exists());
}
#[test]
#[cfg(unix)]
fn default_install_and_explicit_path_edit_only_touch_the_selected_home() {
    let root = tempfile::tempdir().unwrap();
    let profile = root.path().join(".profile");
    fs::write(&profile, "export KEEP_ME=original\n").unwrap();
    let out = command(root.path())
        .env("HOME", root.path())
        .env("SHELL", "/bin/sh")
        .env("PATH", "/usr/bin:/bin")
        .args(["install", "--add-to-path"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(root.path().join(".local/bin/i").is_file());
    let text = fs::read_to_string(&profile).unwrap();
    assert!(text.starts_with("export KEEP_ME=original\n"));
    assert!(text.contains(&format!(
        "export PATH='{}'",
        dunce::canonicalize(root.path().join(".local/bin")).unwrap().display()
    )));
    assert!(
        fs::read_dir(root.path())
            .unwrap()
            .filter_map(Result::ok)
            .any(|e| e.file_name().to_string_lossy().starts_with(".profile.backup-"))
    );
    assert!(!root.path().join("config/config.json").exists());
}
#[test]
fn legacy_migration_is_explicit_and_refuses_custom_changes() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("config");
    fs::create_dir_all(&dir).unwrap();
    let out = command(root.path()).args(["--runtime", "manifest"]).output().unwrap();
    assert!(out.status.success());
    let mut manifest: Value = serde_json::from_slice(&out.stdout).unwrap();
    for section in ["tools", "hooks"] {
        for entry in manifest[section].as_array_mut().unwrap() {
            entry["exec"] = json!(["/retired/repository/target/debug/i-plugins", entry["module"]]);
        }
    }
    manifest["provider"]["exec"] = json!(["/retired/repository/target/debug/i-plugins", "provider"]);
    let legacy = dir.join("desktop.json");
    fs::write(&legacy, manifest.to_string()).unwrap();
    let file = dir.join("config.json");
    fs::write(&file, json!({"plugins":"desktop.json","model":"old"}).to_string()).unwrap();
    assert!(setup(root.path()).arg("--confirm").output().unwrap().status.success());
    assert_eq!(value(&file)["plugins"], "desktop.json");
    let original = fs::read(&legacy).unwrap();
    let out = setup(root.path()).args(["--confirm", "--migrate-builtins"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(value(&file)["plugins"], "builtin.json");
    assert_eq!(fs::read(&legacy).unwrap(), original);
    let mut config = value(&file);
    config["plugins"] = json!("desktop.json");
    fs::write(&file, config.to_string()).unwrap();
    manifest["hooks"][0]["exec"] = json!(["/custom/worker", "different"]);
    fs::write(&legacy, manifest.to_string()).unwrap();
    let before = fs::read(&file).unwrap();
    assert!(!setup(root.path()).args(["--confirm", "--migrate-builtins"]).output().unwrap().status.success());
    assert_eq!(fs::read(&file).unwrap(), before);
}
