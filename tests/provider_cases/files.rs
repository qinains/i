use super::*;
mod gc;
mod lifecycle;
mod management;
fn binary() -> std::ffi::OsString {
    std::env::var_os("I_PROVIDER_TEST_BINARY").unwrap_or_else(|| env!("CARGO_BIN_EXE_i").into())
}
fn configuration(root: &Path, enabled: bool) -> std::path::PathBuf {
    fs::create_dir_all(root.join(".app/config")).unwrap();
    fs::write(
        root.join(".app/config/provider.json"),
        json!({"images":"files","account_scope":"fixture","expires_after_seconds":3600}).to_string(),
    )
    .unwrap();
    let path = root.join("api.json");
    fs::write(
        &path,
        json!({"protocol":"deepseek","provider_operations":enabled,
        "api_key_env":"FILES_TEST_KEY","max_tokens":128,"network_retries":5,
        "extra_body":{"thinking":{"type":"enabled"}}})
        .to_string(),
    )
    .unwrap();
    fs::write(root.join("image.png"), b"\x89PNG\r\n\x1a\nfixture-image").unwrap();
    path
}
fn invoke(root: &Path, base: &str, key: &str, first: bool) -> Output {
    let mut command = Command::new(binary());
    support::isolate(&mut command, root);
    command.args([
        "--config",
        root.join("api.json").to_str().unwrap(),
        "-C",
        root.to_str().unwrap(),
        "--model",
        "fixture",
        "--base-url",
        base,
        "--session",
        root.join("session.json").to_str().unwrap(),
    ]);
    if first {
        command.args(["--image", "image.png"]);
    }
    command.arg("describe").env("FILES_TEST_KEY", key);
    bounded(&mut command)
}
type Captured = (String, Vec<u8>);
fn server(replies: Vec<Option<(u16, Value)>>) -> (String, thread::JoinHandle<(Vec<Captured>, TcpListener)>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let mut requests = Vec::new();
        for reply in replies {
            let deadline = Instant::now() + Duration::from_secs(15);
            let mut stream = loop {
                if let Ok((stream, _)) = listener.accept() {
                    break stream;
                }
                assert!(Instant::now() < deadline, "missing request");
                thread::sleep(Duration::from_millis(5));
            };
            stream.set_nonblocking(false).unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut bytes = Vec::new();
            let mut one = [0];
            while !bytes.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut one).unwrap();
                bytes.push(one[0]);
            }
            let headers = String::from_utf8(bytes).unwrap();
            let length: usize = headers
                .lines()
                .find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case("content-length").then(|| value.trim().parse().unwrap())
                })
                .unwrap_or(0);
            let mut body = vec![0; length];
            stream.read_exact(&mut body).unwrap();
            requests.push((headers, body));
            if let Some((status, body)) = reply {
                let body = body.to_string();
                write!(
                    stream,
                    "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        }
        (requests, listener)
    });
    (base, handle)
}
fn uploaded(id: &str) -> Value {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    json!({"id":id,"expires_at":now+3600})
}
fn answer() -> Value {
    json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":"image answer","reasoning_content":"retained reasoning"}}]})
}
#[test]
fn files_upload_resume_and_credential_rotation_use_real_multipart_and_stable_ids() {
    let root = tempfile::tempdir().unwrap();
    configuration(root.path(), true);
    let (base, server) = server(vec![
        Some((201, uploaded("file-api-one"))),
        Some((200, answer())),
        Some((200, answer())),
        Some((201, uploaded("file-api-two"))),
        Some((200, answer())),
    ]);
    for (key, first) in [("first-secret", true), ("first-secret", false), ("second-secret", false)] {
        let output = invoke(root.path(), &base, key, first);
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        if first {
            fs::remove_file(root.path().join("image.png")).unwrap();
        }
    }
    let (requests, listener) = server.join().unwrap();
    assert_eq!(listener.accept().unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
    for index in [0, 3] {
        let (headers, body) = &requests[index];
        assert!(headers.starts_with("POST /files HTTP/1.1"));
        let boundary = headers.lines().find_map(|line| line.split_once("boundary=").map(|(_, b)| b)).unwrap();
        assert!(body.starts_with(format!("--{boundary}\r\n").as_bytes()));
        assert!(body.ends_with(format!("\r\n--{boundary}--\r\n").as_bytes()));
        assert!(body.windows(8).any(|w| w == b"\x89PNG\r\n\x1a\n"));
        assert!(String::from_utf8_lossy(body).contains("name=\"purpose\"\r\n\r\nuser_data"));
    }
    assert!(requests[0].0.contains("Bearer first-secret"));
    assert!(requests[3].0.contains("Bearer second-secret"));
    let first: Value = serde_json::from_slice(&requests[1].1).unwrap();
    let second: Value = serde_json::from_slice(&requests[2].1).unwrap();
    let third: Value = serde_json::from_slice(&requests[4].1).unwrap();
    assert!(requests[1].0.starts_with("POST /v1/chat/completions HTTP/1.1"));
    assert_eq!(first["max_tokens"], 128);
    assert_eq!(first["thinking"]["type"], "enabled");
    let old = first["messages"].as_array().unwrap();
    assert_eq!(old, &second["messages"].as_array().unwrap()[..old.len()]);
    assert!(second.to_string().contains("retained reasoning"));
    assert!(first.to_string().contains("file-api-one") && third.to_string().contains("file-api-two"));
    assert!(!first.to_string().contains("data:image"));
    let session = fs::read_to_string(root.path().join("session.json")).unwrap();
    assert!(session.contains("data:image/png;base64,"));
    assert!(!session.contains("file-api-one"));
    let registry = fs::read_to_string(support::store(root.path()).join("provider-files.json")).unwrap();
    assert!(!registry.contains("first-secret") && !registry.contains("data:image"));
}
#[test]
fn failed_uploads_are_not_retried_even_across_restart() {
    for reply in [
        None,
        Some((503, json!({"error":"unavailable"}))),
        Some((201, json!({"id":"bad"}))),
        Some((201, json!({"id":"file-api-x"}))),
    ] {
        let root = tempfile::tempdir().unwrap();
        configuration(root.path(), true);
        let (base, server) = server(vec![reply]);
        let output = invoke(root.path(), &base, "test-key", true);
        assert!(!output.status.success());
        let (requests, listener) = server.join().unwrap();
        assert_eq!(requests.len(), 1);
        let retry = invoke(root.path(), &base, "test-key", false);
        assert!(!retry.status.success());
        assert!(String::from_utf8_lossy(&retry.stderr).contains("pending/unknown"));
        assert_eq!(listener.accept().unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
    }
}
#[test]
fn generic_operations_enforce_origin_permission_payload_and_count_limits() {
    for fault in ["disabled", "origin", "oversized", "count"] {
        let root = tempfile::tempdir().unwrap();
        let code = format!(
            r#"import json,sys,base64
p=json.load(sys.stdin); d=p['data']; fault={fault:?}
url=d['base_url']+'/aux'
if fault=='origin': url='http://other.invalid/aux'
body=base64.b64encode(b'x'*(17*1024*1024)).decode() if fault=='oversized' else 'e30='
print(json.dumps({{'version':1,'state':{{}},'operation':{{'url':url,'body_base64':body,'headers':{{}},'auth':None}}}}))"#
        );
        let config = binding(root.path(), &code);
        let mut api: Value = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
        api["provider_operations"] = json!(fault != "disabled");
        fs::write(&config, api.to_string()).unwrap();
        let replies = if fault == "count" { vec![Some((200, json!({}))); 8] } else { vec![] };
        let (base, server) = server(replies);
        let mut command = Command::new(binary());
        support::isolate(&mut command, root.path());
        command.args([
            "--config",
            config.to_str().unwrap(),
            "-C",
            root.path().to_str().unwrap(),
            "--model",
            "fixture",
            "--base-url",
            &base,
            "hello",
        ]);
        let output = bounded(&mut command);
        assert!(!output.status.success(), "{fault}");
        let (requests, listener) = server.join().unwrap();
        assert_eq!(requests.len(), if fault == "count" { 8 } else { 0 });
        assert_eq!(listener.accept().unwrap_err().kind(), std::io::ErrorKind::WouldBlock, "{fault}");
    }
}
#[test]
fn killing_host_after_upload_bytes_does_not_replay_on_restart() {
    let root = tempfile::tempdir().unwrap();
    configuration(root.path(), true);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let mut command = Command::new(binary());
    support::isolate(&mut command, root.path());
    command
        .args([
            "--config",
            root.path().join("api.json").to_str().unwrap(),
            "-C",
            root.path().to_str().unwrap(),
            "--model",
            "fixture",
            "--base-url",
            &base,
            "--session",
            root.path().join("session.json").to_str().unwrap(),
            "--image",
            "image.png",
            "describe",
        ])
        .env("FILES_TEST_KEY", "test-key");
    let mut child = command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut stream = loop {
        if let Ok((stream, _)) = listener.accept() {
            break stream;
        }
        if Instant::now() > deadline {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("missing upload");
        }
        thread::sleep(Duration::from_millis(5));
    };
    stream.set_nonblocking(false).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut bytes = Vec::new();
    let mut one = [0];
    while !bytes.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut one).unwrap();
        bytes.push(one[0]);
    }
    let headers = String::from_utf8(bytes).unwrap();
    assert!(headers.starts_with("POST /files "));
    let length: usize = headers
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case("content-length").then(|| value.trim().parse().unwrap())
        })
        .unwrap();
    let mut body = vec![0; length];
    stream.read_exact(&mut body).unwrap();
    child.kill().unwrap();
    child.wait_with_output().unwrap();
    drop(stream);
    let output = invoke(root.path(), &base, "test-key", false);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("pending/unknown"));
    assert_eq!(listener.accept().unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
}
#[test]
fn files_never_upload_without_explicit_host_permission() {
    let root = tempfile::tempdir().unwrap();
    configuration(root.path(), false);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let output = invoke(root.path(), &format!("http://{}", listener.local_addr().unwrap()), "test-key", true);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("provider_operations=true"));
    assert_eq!(listener.accept().unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
}
