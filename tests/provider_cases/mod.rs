use super::*;
mod files;
fn binding(root: &Path, config: Value) -> std::path::PathBuf {
    let path = root.join("api.json");
    fs::write(
        &path,
        json!({"protocol":"independent-vendor","api_key_env":"PROVIDER_TEST_TOKEN",
        "provider":{"exec":support::fixture("provider",config),"cwd":root}})
        .to_string(),
    )
    .unwrap();
    path
}
fn bounded(command: &mut Command) -> Output {
    let mut child = command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() > deadline {
            child.kill().unwrap();
            panic!("provider test timeout");
        }
        thread::sleep(Duration::from_millis(10));
    }
    child.wait_with_output().unwrap()
}
#[test]
fn independent_provider_owns_wire_format_but_never_receives_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let config = binding(dir.path(), json!({"kind":"independent"}));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut stream = loop {
            if let Ok((stream, _)) = listener.accept() {
                break stream;
            }
            assert!(Instant::now() < deadline, "no provider request");
            thread::sleep(Duration::from_millis(10));
        };
        stream.set_nonblocking(false).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut headers = Vec::new();
        let mut byte = [0];
        while !headers.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            headers.push(byte[0]);
        }
        let headers = String::from_utf8(headers).unwrap();
        let length: usize = headers
            .lines()
            .find_map(|line| {
                let (k, v) = line.split_once(':')?;
                k.eq_ignore_ascii_case("content-length").then(|| v.trim().parse().unwrap())
            })
            .unwrap();
        let mut body = vec![0; length];
        stream.read_exact(&mut body).unwrap();
        let reply = r#"{"answer":"custom result"}"#;
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len())
            .unwrap();
        (headers, serde_json::from_slice::<Value>(&body).unwrap())
    });
    let mut command = Command::new(env!("CARGO_BIN_EXE_i"));
    support::isolate(&mut command, dir.path());
    command
        .args([
            "--config",
            config.to_str().unwrap(),
            "-C",
            dir.path().to_str().unwrap(),
            "--base-url",
            &base,
            "--model",
            "fixture",
            "hello",
        ])
        .env("PROVIDER_TEST_TOKEN", "fixture-token")
        .env("NO_PROXY", "127.0.0.1")
        .env("no_proxy", "127.0.0.1");
    let output = bounded(&mut command);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).contains("custom result"));
    let (headers, body) = server.join().unwrap();
    assert!(headers.starts_with("POST /v1/custom HTTP/1.1"));
    assert!(headers.to_lowercase().contains("x-provider-key: Key fixture-token".to_lowercase().as_str()));
    assert!(!headers.to_lowercase().contains("authorization:"));
    assert_eq!(body, json!({"question":"hello","cap":8192}));
}
#[test]
fn provider_failures_cross_origin_and_reserved_headers_never_send_http_or_fall_back() {
    for fault in ["failure", "origin", "header", "auth_name", "auth_prefix", "auth_collision", "version", "missing"] {
        let dir = tempfile::tempdir().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}/v1", listener.local_addr().unwrap());
        let config = binding(dir.path(), json!({"kind":"fault","fault":fault}));
        if fault == "missing" {
            fs::write(&config, json!({"provider":{"exec":[dir.path().join("missing-worker")]}}).to_string()).unwrap();
        }
        let output = run(dir.path(), &base, &["--config", config.to_str().unwrap(), "hello"]);
        assert!(!output.status.success(), "accepted {fault}");
        assert_eq!(listener.accept().unwrap_err().kind(), std::io::ErrorKind::WouldBlock, "sent HTTP for {fault}");
    }
}
#[test]
fn native_responses_provider_works_without_the_legacy_responses_hook() {
    let dir = tempfile::tempdir().unwrap();
    let reply = json!({"status":"completed","output":[{"type":"message","role":"assistant",
        "content":[{"type":"output_text","text":"native response"}]}]});
    let (base, server) = server_route(vec![(200, reply)], "responses");
    let output = run(dir.path(), &base, &["--protocol", "responses", "hello"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let requests = server.join().unwrap();
    assert_eq!(requests[0]["store"], false);
    assert!(requests[0].get("messages").is_none());
    assert!(String::from_utf8_lossy(&output.stdout).contains("native response"));
}
