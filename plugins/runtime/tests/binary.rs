mod binding_cases;
use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn console_plugins_and_login_are_local_commands_without_a_model() {
    let root = tempfile::tempdir().unwrap();
    let mut child = command(root.path())
        .args(["console", "--no-logo", "-C", root.path().to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"/plugins\n/login\n/cancel\n/exit\n").unwrap();
    let output = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{text}");
    assert!(text.contains("ChatGPT") && text.contains("Console API"), "{text}");
    assert!(text.contains("/plugins install chatgpt"), "{text}");
    assert!(text.contains("No enabled login plugins"), "{text}");
    assert!(!text.contains("[task:"), "slash commands must never be model tasks: {text}");
    assert!(!root.path().join("config/config.json").exists());
}

#[test]
fn console_plugin_install_requires_confirmation_and_can_cancel() {
    let root = tempfile::tempdir().unwrap();
    let mut child = command(root.path())
        .args(["console", "--no-logo"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"/plugins install chatgpt\nn\n/exit\n").unwrap();
    let output = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{text}");
    assert!(text.contains("native code") && text.contains("[y/N]"), "{text}");
    assert!(text.contains("cancelled"), "{text}");
    assert!(!root.path().join("data/connections").exists());
}

#[test]
fn start_without_configuration_can_open_plugins_before_setup() {
    let root = tempfile::tempdir().unwrap();
    let mut child = command(root.path())
        .args(["start", "--no-logo"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"/plugins\n/cancel\n/exit\n").unwrap();
    let output = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{text}");
    assert!(text.contains("ChatGPT"), "{text}");
    assert!(!root.path().join("config/config.json").exists());
}

#[test]
fn plugin_cli_lists_and_cancels_install_without_external_helper_commands() {
    let root = tempfile::tempdir().unwrap();
    let list = command(root.path()).args(["plugins", "list"]).output().unwrap();
    assert!(list.status.success(), "{}", String::from_utf8_lossy(&list.stderr));
    assert!(String::from_utf8_lossy(&list.stdout).contains("ChatGPT"));
    let mut child = command(root.path())
        .args(["plugins", "install", "claude"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"n\n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).contains("cancelled"));
    assert!(!root.path().join("data/connections").exists());
}

fn command(root: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_i-plugins"));
    for (key, name) in
        [("I_CONFIG_DIR", "config"), ("I_DATA_DIR", "data"), ("I_CACHE_DIR", "cache"), ("I_RUNTIME_DIR", "runtime")]
    {
        command.env(key, root.join(name));
    }
    command
}
#[test]
fn console_proxy_switches_locally_without_a_model_or_configuration_write() {
    let root = tempfile::tempdir().unwrap();
    let mut child = command(root.path())
        .args(["console", "--no-logo"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            b"/proxy 3067\n/proxy\n/proxy socks5://127.0.0.1:3067\n/proxy no\n/proxy noproxy\n/proxy env\n/exit\n",
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{text}");
    assert!(text.contains("http://127.0.0.1:3067"), "{text}");
    assert!(text.contains("socks5://127.0.0.1:3067"), "{text}");
    assert!(text.contains("direct") && text.contains("environment"), "{text}");
    assert!(!text.contains("[task:"), "{text}");
    assert!(!root.path().join("config/config.json").exists());
}
#[test]
fn desktop_init_preserves_files_and_separates_portable_and_executable_configuration() {
    let root = tempfile::tempdir().unwrap();
    assert!(command(root.path()).arg("init").output().unwrap().status.success());
    let config = root.path().join("config");
    let portable = std::fs::read_to_string(config.join("plugins.json")).unwrap();
    assert!(!portable.contains("exec") && portable.contains("standard"));
    let manifest: Value = serde_json::from_slice(&std::fs::read(config.join("desktop.json")).unwrap()).unwrap();
    for entry in manifest["hooks"].as_array().unwrap() {
        assert_ne!(entry["exec"][1], "responses");
        assert_eq!(entry["cwd"], config.to_string_lossy().as_ref());
    }
    std::fs::write(config.join("config.json"), r#"{"model":"keep-me"}"#).unwrap();
    assert!(command(root.path()).arg("init").output().unwrap().status.success());
    assert!(std::fs::read_to_string(config.join("config.json")).unwrap().contains("keep-me"));
}
#[test]
fn console_rejects_missing_values_before_initializing_storage() {
    for args in [
        vec!["--config"],
        vec!["--plugins"],
        vec!["-C"],
        vec!["--session-name"],
        vec!["--config", "--session-name", "test"],
        vec!["--plugins="],
        vec!["--config", ""],
    ] {
        let root = tempfile::tempdir().unwrap();
        let output = command(root.path()).arg("console").args(&args).stdin(Stdio::null()).output().unwrap();
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("requires a value"), "{args:?}: {error}");
        assert!(!error.contains("[session:") && !error.contains("lninl / i"));
        assert!(!root.path().join("data").exists());
    }
}
#[test]
fn console_accepts_equals_arguments_and_starts_offline_without_a_task() {
    let root = tempfile::tempdir().unwrap();
    let api = root.path().join("api.json");
    let manifest = root.path().join("plugins.json");
    std::fs::write(&api, "{}").unwrap();
    std::fs::write(&manifest, r#"{"tools":[],"hooks":[]}"#).unwrap();
    let output = command(root.path())
        .args(["console", "--no-logo", "--session-name=test"])
        .arg(format!("--config={}", api.display()))
        .arg(format!("--plugins={}", manifest.display()))
        .arg(format!("--dir={}", root.path().display()))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{error}");
    assert!(error.contains("test.json") && error.contains("Type a task"));
    assert!(!error.contains("lninl / i"));
}
#[test]
fn console_auto_aliases_start_in_auto_and_old_yes_is_rejected() {
    for flag in ["--yolo", "--dangerously-skip-permissions", "--yes"] {
        let root = tempfile::tempdir().unwrap();
        let out = command(root.path())
            .args(["console", "--no-logo", flag, "-C", root.path().to_str().unwrap()])
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let error = String::from_utf8_lossy(&out.stderr);
        if flag == "--yes" {
            assert!(!out.status.success());
            assert!(error.contains("was removed"));
            assert!(!root.path().join("data").exists());
        } else {
            assert!(out.status.success(), "{error}");
            assert!(error.contains("mode: auto"), "{error}");
        }
    }
}
#[test]
fn console_rejects_no_argument_command_arguments_without_starting_backend() {
    let root = tempfile::tempdir().unwrap();
    let mut child = command(root.path())
        .args(["console", "--no-logo", "--bin", "/nonexistent-backend", "-C", root.path().to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"/help extra\n/approve garbage\n/deny x\n/exit x\n/mode auto nope\n/mode status\n/help\n/exit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{error}");
    assert_eq!(error.matches("invalid slash command arguments").count(), 4, "{error}");
    assert!(error.contains("usage: /mode") && error.contains("Show available commands"), "{error}");
    assert_eq!(error.matches("[mode: ask;").count(), 2, "{error}");
    assert!(
        !error.contains("[task:") && !error.contains("steering queued") && !error.contains("mode: auto"),
        "{error}"
    );
}
#[cfg(unix)]
#[test]
fn console_wrong_arguments_preserve_a_pending_approval() {
    use std::{
        os::unix::fs::PermissionsExt,
        thread,
        time::{Duration, Instant},
    };
    let root = tempfile::tempdir().unwrap();
    let backend = root.path().join("backend.sh");
    let ready = root.path().join("ready");
    let answer = root.path().join("answer");
    std::fs::write(&backend, "#!/bin/sh\nprintf '\\036%s\\036' \"$I_APPROVAL_TOKEN\" >&2\nprintf ready > \"$FIXTURE_READY\"\nIFS= read -r answer\nprintf '%s' \"$answer\" > \"$FIXTURE_ANSWER\"\n").unwrap();
    std::fs::set_permissions(&backend, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut child = command(root.path())
        .args([
            "console",
            "--no-logo",
            "--task",
            "fixture",
            "--bin",
            backend.to_str().unwrap(),
            "-C",
            root.path().to_str().unwrap(),
        ])
        .env("FIXTURE_READY", &ready)
        .env("FIXTURE_ANSWER", &answer)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ready.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(ready.exists(), "fixture backend never became ready");
    thread::sleep(Duration::from_millis(100));
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"/approve garbage\n/deny x\n/help x\n/exit x\n/mode auto nope\n/mode status\n/approve\n/exit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{error}");
    assert_eq!(error.matches("invalid slash command arguments").count(), 4, "{error}");
    assert_eq!(error.matches("[mode: ask;").count(), 2, "{error}");
    assert!(
        !error.contains("no pending approval") && !error.contains("steering queued") && !error.contains("mode: auto"),
        "{error}"
    );
    assert_eq!(std::fs::read_to_string(answer).unwrap(), "y");
}
#[test]
fn native_worker_serves_multiple_frames_and_reports_errors_with_ids() {
    let root = tempfile::tempdir().unwrap();
    let mut child = command(root.path())
        .env_remove("HOME")
        .env_remove("USERPROFILE")
        .arg("prompts")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    for id in 1..=3 {
        let data = if id == 2 { json!([]) } else { json!({"seed":id}) };
        writeln!(input, "{}", json!({"id":id,"event":"future_event","workspace":root.path(),"data":data})).unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let frames: Vec<Value> =
        String::from_utf8(output.stdout).unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert_eq!(frames.len(), 3);
    assert_eq!(frames[0]["data"]["seed"], 1);
    assert_eq!(frames[1]["id"], 2);
    assert!(frames[1]["error"].is_string());
    assert_eq!(frames[2]["data"]["seed"], 3);
}
#[test]
fn production_plugins_run_without_interpreters_or_path() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("main.rs"), "needle").unwrap();
    for (plugin, event, data) in [
        (
            "budget",
            "context_budget",
            json!({"history":[],"model":"fixture","max_tokens":100,"policy":{"context_window":1000}}),
        ),
        ("compact", "before_compact", json!({"messages":[],"policy":{"images":"omit"}})),
        ("responses", "after_response", json!({"response":{"status":"completed","output":[]}})),
        ("memory", "before_turn", json!({"messages":[{"role":"system","content":"base"}]})),
        (
            "cache",
            "before_request",
            json!({"url":"https://provider.invalid/v1/chat/completions","history":[],
            "body":{"model":"fixture","messages":[],"tools":[]}}),
        ),
        ("prompts", "system_prompt", json!({"prompt":"base"})),
        ("search", "tool", json!({"name":"search","arguments":{"query":"needle"}})),
        ("skills", "before_turn", json!({"messages":[{"role":"system","content":"base"}]})),
        ("steering", "before_turn", json!({"messages":[{"role":"system","content":"base"}]})),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_i-plugins"));
        command
            .arg(plugin)
            .env_clear()
            .env("PATH", "")
            .envs(
                ["SystemRoot", "TEMP", "TMP"]
                    .into_iter()
                    .filter_map(|key| std::env::var_os(key).map(|value| (key, value))),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if plugin == "steering" {
            command.arg("--queue").arg(root.path().join("inbox.json"));
        }
        let mut child = command.spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(
                format!(
                    "{}\n",
                    json!({"id":1,"event":event,"workspace":root.path(),"data":data,
                "paths":{"config_dir":root.path().join("config"),"data_dir":root.path().join("data"),
                    "cache_dir":root.path().join("cache"),"runtime_dir":root.path().join("runtime")}})
                )
                .as_bytes(),
            )
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "{plugin}: {}", String::from_utf8_lossy(&output.stderr));
        let reply: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(reply["id"], 1);
        assert!(reply["data"].is_object(), "{reply}");
    }
}

#[test]
fn executable_generates_self_contained_manifest() {
    let root = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_i-plugins"))
        .env("I_CONFIG_DIR", root.path().join("config"))
        .args(["manifest", "full"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["tools"].as_array().unwrap().len(), 8);
    for section in ["tools", "hooks"] {
        for item in value[section].as_array().unwrap() {
            assert!(std::path::Path::new(item["exec"][0].as_str().unwrap()).is_absolute());
            assert_eq!(
                std::path::Path::new(item["exec"][0].as_str().unwrap()).canonicalize().unwrap(),
                std::path::Path::new(env!("CARGO_BIN_EXE_i-plugins")).canonicalize().unwrap()
            );
        }
    }
}
