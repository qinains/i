use super::*;

fn manifest(root: &Path, reply: Option<Value>) -> std::path::PathBuf {
    let policy = match reply {
        Some(reply) => format!(
            "import json,sys\np=json.load(sys.stdin)\nassert p['event']=='tool_approval'\nassert set(p['data'])=={{'name','arguments','external','decision'}}\nprint({:?})",
            reply.to_string()
        ),
        None => "raise RuntimeError('policy unavailable')".into(),
    };
    let tool = support::python(
        "from pathlib import Path\nimport json\nPath('external-ran').write_text('bad')\nprint(json.dumps({'output':'executed'}))",
    );
    let path = root.join("plugins.json");
    fs::write(&path, json!({"tools":[{"name":"external_action","description":"test","parameters":{"type":"object"},
        "exec":["python3","-c",tool]}],"hooks":[{"events":["tool_approval"],"exec":["python3","-c",support::python(&policy)]}]}).to_string()).unwrap();
    path
}
#[test]
fn policy_decisions_are_enforced_including_ask_despite_yes_and_failures_never_fall_back() {
    for reply in [
        Some(json!({"decision":"allow","name":"shell","arguments":{"command":"must not replace arguments"}})),
        Some(json!({"decision":"ask"})),
        Some(json!({"decision":"deny"})),
        Some(json!({"decision":"invalid"})),
        Some(json!({"decision":true})),
        Some(json!({})),
        None,
    ] {
        let dir = tempfile::tempdir().unwrap();
        let allow = reply.as_ref().is_some_and(|v| v["decision"] == "allow");
        let ask = reply.as_ref().is_some_and(|v| v["decision"] == "ask");
        let manifest = manifest(dir.path(), reply.clone());
        let (base, server) = server(vec![
            (200, tool_reply("write", json!({"path":"written.txt","content":"original arguments"}))),
            (200, final_reply()),
        ]);
        let mut args = vec!["--plugins", manifest.to_str().unwrap()];
        if !allow {
            args.push("--dangerously-skip-permissions");
        }
        args.push("test approval");
        let output = run(dir.path(), &base, &args);
        assert!(output.status.success(), "{reply:?}: {}", String::from_utf8_lossy(&output.stderr));
        let requests = server.join().unwrap();
        let result = requests[1]["messages"].as_array().unwrap().iter().find(|v| v["role"] == "tool").unwrap();
        assert_eq!(result["content"].as_str().unwrap().starts_with("ERROR:"), !allow, "{reply:?}");
        assert_eq!(dir.path().join("written.txt").exists(), allow, "{reply:?}");
        assert_eq!(String::from_utf8_lossy(&output.stderr).contains("Allow?"), ask, "{reply:?}");
        if allow {
            assert_eq!(fs::read_to_string(dir.path().join("written.txt")).unwrap(), "original arguments");
        }
    }
}
#[test]
fn console_uses_replacement_approval_with_mode_file_and_never_falls_back() {
    let python = Command::new("python3").args(["-c", "import sys; print(sys.executable)"]).output().unwrap();
    assert!(python.status.success());
    let python = String::from_utf8(python.stdout).unwrap().trim().to_owned();
    for missing in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join(".app/config");
        fs::create_dir_all(&config).unwrap();
        let body = support::python(
            "import json,sys\nfrom pathlib import Path\np=json.load(sys.stdin)\nmode=json.loads(Path(sys.argv[sys.argv.index('--mode-file')+1]).read_text())['mode']\nPath('policy-seen.json').write_text(json.dumps({'mode':mode,'default':p['data']['decision']}))\np['data']['decision']='deny'\nprint(json.dumps(p['data']))",
        );
        let exec = if missing { json!([dir.path().join("missing-worker")]) } else { json!([python, "-c", body]) };
        fs::write(config.join("bindings.json"), json!({"approval":{"exec":exec,"cwd":dir.path()}}).to_string())
            .unwrap();
        let (base, server) = server(vec![
            (200, tool_reply("write", json!({"path":"must-not-exist","content":"bad"}))),
            (200, final_reply()),
        ]);
        let mut command = Command::new(plugin_binary());
        support::isolate(&mut command, dir.path());
        let mut child = command
            .args([
                "console",
                "--no-logo",
                "--bin",
                env!("CARGO_BIN_EXE_i"),
                "-C",
                dir.path().to_str().unwrap(),
                "--base-url",
                &base,
                "--model",
                "test-model",
                "--yolo",
                "--task",
                "test",
            ])
            .env("NO_COLOR", "1")
            .env("NO_PROXY", "127.0.0.1")
            .env("no_proxy", "127.0.0.1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() > deadline {
                child.kill().unwrap();
                panic!("console timeout");
            }
            thread::sleep(Duration::from_millis(10));
        }
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let requests = server.join().unwrap();
        assert!(requests[1]["messages"].to_string().contains("ERROR:"));
        assert!(!dir.path().join("must-not-exist").exists());
        if !missing {
            let seen: Value = serde_json::from_slice(&fs::read(dir.path().join("policy-seen.json")).unwrap()).unwrap();
            assert_eq!(seen, json!({"mode":"auto","default":"ask"}));
        }
    }
}
#[test]
fn policy_can_deny_native_reads_shell_and_external_tools_before_execution() {
    for (name, args) in [
        ("read", json!({"path":"private.txt"})),
        ("shell", json!({"command":"echo bad > shell-ran"})),
        ("external_action", json!({})),
    ] {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("private.txt"), "private-content-sentinel").unwrap();
        let manifest = manifest(dir.path(), Some(json!({"decision":"deny"})));
        let (base, server) = server(vec![(200, tool_reply(name, args)), (200, final_reply())]);
        let output = run(dir.path(), &base, &["--plugins", manifest.to_str().unwrap(), "--yolo", "test denial"]);
        assert!(output.status.success());
        let requests = server.join().unwrap();
        assert!(requests[1]["messages"].to_string().contains("approval denied"));
        assert!(!requests[1].to_string().contains("private-content-sentinel"));
        assert!(!dir.path().join("shell-ran").exists());
        assert!(!dir.path().join("external-ran").exists());
    }
}
