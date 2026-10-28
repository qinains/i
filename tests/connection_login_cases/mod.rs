use super::*;
use sha2::{Digest, Sha256};

fn setup(root: &Path, deny: bool) -> std::path::PathBuf {
    let app = root.join(".app");
    let data = app.join("data");
    let config = app.join("config");
    fs::create_dir_all(&data).unwrap();
    fs::create_dir_all(&config).unwrap();
    let fixture = support::fixture("unused", json!({}));
    let exe = data.join(format!("connection-fixture{}", std::env::consts::EXE_SUFFIX));
    fs::copy(fixture[0].as_str().unwrap(), &exe).unwrap();
    let digest = format!("{:x}", Sha256::digest(fs::read(&exe).unwrap()));
    fs::write(
        config.join("native-plugins.json"),
        json!({"chatgpt":{"enabled":true,"executable":exe.file_name().unwrap().to_str().unwrap(),"sha256":digest}})
            .to_string(),
    )
    .unwrap();
    if deny {
        let auth = data.join("connections/accounts/chatgpt");
        fs::create_dir_all(&auth).unwrap();
        fs::write(auth.join("deny-login"), "fixture").unwrap();
        fs::write(data.join("deny-login"), "fixture").unwrap();
    }
    config.join("config.json")
}
fn console(root: &Path, input: &[u8]) -> Output {
    let mut cmd = Command::new(plugin_binary());
    support::isolate(&mut cmd, root);
    let mut child = cmd
        .args(["console", "--bin", env!("CARGO_BIN_EXE_i"), "--no-logo", "-C", root.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::BufRead;
    let stderr = child.stderr.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        for line in std::io::BufReader::new(stderr).lines() {
            let line = line.unwrap();
            if line.contains("Select a model")
                || line.contains("Choose a model number")
                || line.contains("plugin operation failed")
            {
                let _ = tx.send(());
            }
            bytes.extend_from_slice(line.as_bytes());
            bytes.push(b'\n');
        }
        bytes
    });
    let mut stdin = child.stdin.take().unwrap();
    let prefix = if input.starts_with(b"/login chatgpt\n") {
        Some(b"/login chatgpt\n".as_slice())
    } else if input.starts_with(b"/model\n") {
        Some(b"/model\n".as_slice())
    } else {
        None
    };
    if let Some(prefix) = prefix {
        stdin.write_all(prefix).unwrap();
        if let Err(error) = rx.recv_timeout(Duration::from_secs(15)) {
            let _ = child.kill();
            let _ = child.wait();
            drop(stdin);
            panic!("login result prompt: {error}; {}", String::from_utf8_lossy(&reader.join().unwrap()));
        }
        stdin.write_all(&input[prefix.len()..]).unwrap();
    } else {
        stdin.write_all(input).unwrap();
    }
    drop(stdin);
    let mut output = child.wait_with_output().unwrap();
    output.stderr = reader.join().unwrap();
    output
}
#[test]
fn unified_login_selects_model_and_executes_next_task_without_manual_config() {
    let root = tempfile::tempdir().unwrap();
    let file = setup(root.path(), false);
    let output = console(root.path(), b"/login chatgpt\n1\nhello\n/exit\n");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let value: Value = serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
    assert_eq!(value["model"], "fixture-model");
    assert_eq!(value["protocol"], "responses");
    assert_eq!(value["provider"]["managed_connection"], "chatgpt");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("fixture connection reply"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[cfg(feature = "bundled")]
#[test]
fn scoped_chatgpt_proxy_reaches_login_models_and_cleared_environment_worker() {
    let root = tempfile::tempdir().unwrap();
    setup(root.path(), false);
    let app = root.path().join(".app");
    fs::write(app.join("data/expected-proxy"), "socks5://127.0.0.1:3067").unwrap();
    fs::write(
        app.join("config/proxy.json"),
        json!({"default":"no","chatgpt":"socks5://127.0.0.1:3067","model":"no"}).to_string(),
    )
    .unwrap();
    let output = console(root.path(), b"/login chatgpt\n1\nhello\n/exit\n");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).contains("fixture connection reply"));
    assert_eq!(fs::read_to_string(app.join("data/proxy-phases")).unwrap(), "login\nmodels\nworker\n");
}
#[cfg(feature = "bundled")]
#[test]
fn login_with_retired_generated_search_manifest_can_execute_and_restart() {
    let root = tempfile::tempdir().unwrap();
    let file = setup(root.path(), false);
    let config = file.parent().unwrap();
    let manifest = config.join("desktop.json");
    let legacy = json!({"tools":[
        {"name":"search","description":"retired search","parameters":{"type":"object"},"exec":["/retired/i-plugins","search"],"cwd":config},
        {"name":"find_files","description":"retired glob","parameters":{"type":"object"},"exec":["/retired/i-plugins","search"],"cwd":config}],
        "hooks":[{"events":["session_load"],"exec":["/retired/i-plugins","cache"],"cwd":config}]});
    fs::write(&manifest, legacy.to_string()).unwrap();
    fs::write(&file, json!({"plugins":"desktop.json"}).to_string()).unwrap();
    for input in [b"/login chatgpt\n1\nhello\n/exit\n".as_slice(), b"hello again\n/exit\n"] {
        let output = console(root.path(), input);
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("fixture connection reply"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(&manifest).unwrap()).unwrap(), legacy);
}
#[cfg(feature = "bundled")]
#[test]
fn model_menu_uses_existing_login_and_effort_reaches_the_connection_worker() {
    let root = tempfile::tempdir().unwrap();
    let file = setup(root.path(), false);
    let output = console(root.path(), b"/login chatgpt\n1\n/exit\n");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let before: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    fs::write(
        root.path().join(".app/data/expected-model.json"),
        json!({"model":"fixture-model","effort":"high"}).to_string(),
    )
    .unwrap();
    let output = console(root.path(), b"/model\n1\n/model effort high\nhello\n/exit\n");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("fixture connection reply"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let after: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    assert_eq!(after["provider"], before["provider"]);
    assert_eq!(after["extra_body"]["reasoning"]["effort"], "high");
    let output = console(root.path(), b"/model status\n/model effort auto\n/exit\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("Thinking effort: high"));
    let after: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    assert!(after["extra_body"].get("reasoning").is_none());
    assert_eq!(after["provider"], before["provider"]);
}
#[test]
fn unified_login_failure_or_model_cancel_preserves_existing_config() {
    for (deny, input) in
        [(true, b"/login chatgpt\n/exit\n".as_slice()), (false, b"/login chatgpt\n/cancel\n/exit\n".as_slice())]
    {
        let root = tempfile::tempdir().unwrap();
        let file = setup(root.path(), deny);
        let original = b"{\"model\":\"old-model\",\"base_url\":\"https://example.invalid\"}";
        fs::write(&file, original).unwrap();
        let output = console(root.path(), input);
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(fs::read(file).unwrap(), original);
        if deny {
            assert!(String::from_utf8_lossy(&output.stderr).contains("HTTP 401"));
        }
    }
}

#[test]
fn cancelling_model_selection_does_not_change_existing_account_credentials() {
    let root = tempfile::tempdir().unwrap();
    let file = setup(root.path(), false);
    let auth = root.path().join(".app/data/connections/accounts/chatgpt");
    fs::create_dir_all(&auth).unwrap();
    fs::write(auth.join("active-account"), "account-A").unwrap();
    let original=json!({"model":"old","base_url":"https://example.invalid","provider":{"exec":["fixture","--data-dir",auth,"worker"]}}).to_string();
    fs::write(&file, &original).unwrap();
    let output = console(root.path(), b"/login chatgpt\n/cancel\n/exit\n");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(fs::read_to_string(auth.join("active-account")).unwrap(), "account-A");
    assert_eq!(fs::read_to_string(file).unwrap(), original);
}

#[test]
fn oauth_wait_can_be_cancelled_without_waiting_for_callback_timeout() {
    let root = tempfile::tempdir().unwrap();
    setup(root.path(), false);
    fs::write(root.path().join(".app/data/slow-login"), "fixture").unwrap();
    let mut cmd = Command::new(plugin_binary());
    support::isolate(&mut cmd, root.path());
    let mut child = cmd
        .args(["console", "--no-logo", "-C", root.path().to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    child.stdin.take().unwrap().write_all(b"/login chatgpt\n/cancel\n/exit\n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(start.elapsed() < Duration::from_secs(3), "cancellation blocked on native OAuth child");
    assert!(String::from_utf8_lossy(&output.stderr).contains("login cancelled"));
}
