use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};
mod support;
#[test]
fn https_dispatch_sends_client_hello_instead_of_panicking() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("https://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut stream = accept(&listener);
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut header = [0; 5];
        stream.read_exact(&mut header).ok().map(|()| header)
    });
    let result = run(root.path(), &endpoint, json!({"proxy":"","network_retries":0}));
    let observed = server.join().unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert_eq!(result.status.code(), Some(1), "{stderr}");
    assert!(!stderr.contains("panicked") && !stderr.contains("feature is not enabled"), "{stderr}");
    let header = observed.expect("HTTPS must start a TLS handshake, not close TCP before ClientHello");
    assert_eq!(&header[..2], &[0x16, 0x03]);
}
fn request(stream: &mut TcpStream) -> Value {
    stream.set_read_timeout(Some(Duration::from_secs(8))).unwrap();
    let mut reader = BufReader::new(stream);
    let mut length = 0;
    loop {
        let mut line = String::new();
        assert!(reader.read_line(&mut line).unwrap() > 0);
        if line == "\r\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().unwrap();
        }
    }
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
fn answer() -> Value {
    json!({"choices":[{"message":{"role":"assistant","content":"recovered"},"finish_reason":"stop"}],
        "usage":{"prompt_tokens":100,"completion_tokens":5,"prompt_tokens_details":{"cached_tokens":50}}})
}
fn reply(stream: &mut TcpStream, status: u16, value: &Value, extra: &str) {
    let body = value.to_string();
    write!(stream,"HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{body}",body.len()).unwrap();
}
fn accept(listener: &TcpListener) -> TcpStream {
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false).unwrap();
                return stream;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(10))
            }
            error => panic!("accept: {error:?}"),
        }
    }
}
fn run(root: &Path, base: &str, mut config: Value) -> Output {
    run_observed(root, base, config.take(), None)
}
fn run_observed(root: &Path, base: &str, mut config: Value, retry: Option<std::sync::mpsc::Sender<()>>) -> Output {
    let proxy_env = config.as_object_mut().unwrap().remove("_test_proxy_env");
    config["base_url"] = json!(base);
    config["model"] = json!("fixture");
    config["api_key_env"] = json!("I_TEST_UNSET_KEY");
    let path = root.join("api.json");
    fs::write(&path, config.to_string()).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_i"));
    support::isolate(&mut command, root);
    if let Some(proxy) = proxy_env {
        command.env("ALL_PROXY", proxy.as_str().unwrap());
    }
    command
        .args([
            "--config",
            path.to_str().unwrap(),
            "-C",
            root.to_str().unwrap(),
            "--session-name",
            "net",
            "--yolo",
            "task",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    let mut out = child.stdout.take().unwrap();
    let err = child.stderr.take().unwrap();
    let out = thread::spawn(move || {
        let mut b = Vec::new();
        out.read_to_end(&mut b).unwrap();
        b
    });
    let err = thread::spawn(move || {
        let mut b = Vec::new();
        let mut reader = BufReader::new(err);
        loop {
            let mut line = Vec::new();
            if reader.read_until(b'\n', &mut line).unwrap() == 0 {
                break;
            }
            if String::from_utf8_lossy(&line).contains("network retry")
                && let Some(sender) = &retry
            {
                let _ = sender.send(());
            }
            b.extend_from_slice(&line);
        }
        b
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            panic!("network test timed out");
        }
        thread::sleep(Duration::from_millis(10));
    };
    Output { status, stdout: out.join().unwrap(), stderr: err.join().unwrap() }
}
#[test]
fn disconnect_after_tool_retries_identical_request_without_replaying_tool_or_hiding_usage() {
    let root = tempfile::tempdir().unwrap();
    let worker = support::python(
        "import json,sys\nfrom pathlib import Path\ne=json.load(sys.stdin); p=Path(e['workspace'])/'calls.txt'\nwith p.open('a', newline='') as f: f.write('once\\n')\njson.dump({'output':'done'},sys.stdout)",
    );
    fs::write(root.path().join("worker.py"), worker).unwrap();
    let mut manifest: Value = serde_json::from_str(include_str!("../plugins/cache.json")).unwrap();
    for hook in manifest["hooks"].as_array_mut().unwrap() {
        hook["exec"][0] =
            json!(format!("{}/target/debug/i-plugins{}", env!("CARGO_MANIFEST_DIR"), std::env::consts::EXE_SUFFIX));
    }
    manifest["tools"] =
        json!([{"name":"once","description":"test","parameters":{"type":"object"},"exec":["python3","worker.py"]}]);
    let path = root.path().join("plugins.json");
    fs::write(&path, manifest.to_string()).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut stream = accept(&listener);
        request(&mut stream);
        reply(
            &mut stream,
            200,
            &json!({"choices":[{"finish_reason":"tool_calls","message":{"role":"assistant","content":null,
            "tool_calls":[{"id":"once","type":"function","function":{"name":"once","arguments":"{}"}}]}}],
            "usage":{"prompt_tokens":100,"completion_tokens":5,"prompt_tokens_details":{"cached_tokens":0}}}),
            "",
        );
        drop(stream);
        let mut stream = accept(&listener);
        let first = request(&mut stream);
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n{{\"choices\":").unwrap();
        drop(stream);
        let mut stream = accept(&listener);
        let second = request(&mut stream);
        assert_eq!(first, second);
        reply(&mut stream, 200, &answer(), "");
    });
    let output = run(root.path(), &base, json!({"network_retries":1,"plugins":path}));
    server.join().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(fs::read_to_string(root.path().join("calls.txt")).unwrap(), "once\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("network retry 1/1"));
    let saved: Value = serde_json::from_slice(&fs::read(root.path().join(".i/sessions/net.json")).unwrap()).unwrap();
    assert_eq!(saved["messages"].as_array().unwrap().iter().filter(|m| m["role"] == "tool").count(), 1);
    let metrics: Value =
        serde_json::from_slice(&fs::read(support::store(root.path()).join("metrics/cache-stats.json")).unwrap())
            .unwrap();
    assert_eq!(metrics["totals"]["prepared_requests"], 3);
    assert_eq!(metrics["totals"]["observed_responses"], 2);
}
#[test]
fn retry_after_and_exhaustion_are_bounded_and_checkpoint_is_preserved() {
    for recover in [true, false] {
        let root = tempfile::tempdir().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            for index in 0..2 {
                let mut stream = accept(&listener);
                request(&mut stream);
                reply(&mut stream, if recover && index == 1 { 200 } else { 503 }, &answer(), "Retry-After: 2\r\n");
            }
            listener.set_nonblocking(true).unwrap();
            assert!(listener.accept().is_err());
        });
        let start = Instant::now();
        let output = run(root.path(), &base, json!({"network_retries":1}));
        server.join().unwrap();
        assert_eq!(output.status.success(), recover, "{}", String::from_utf8_lossy(&output.stderr));
        assert!(start.elapsed() >= Duration::from_secs(2));
        let saved: Value =
            serde_json::from_slice(&fs::read(root.path().join(".i/sessions/net.json")).unwrap()).unwrap();
        assert_eq!(saved["messages"].as_array().unwrap().len(), if recover { 3 } else { 2 });
    }
}
#[test]
fn permanent_http_errors_redirects_and_disabled_retries_make_only_one_request() {
    for (status, retries) in [(400, 5), (401, 5), (403, 5), (302, 5), (503, 0), (429, 5)] {
        let root = tempfile::tempdir().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let mut stream = accept(&listener);
            request(&mut stream);
            reply(
                &mut stream,
                status,
                &json!({"error":"fixture"}),
                "Location: /must-not-follow\r\nRetry-After: 999\r\n",
            );
        });
        let output = run(root.path(), &base, json!({"network_retries":retries}));
        server.join().unwrap();
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("network retry"));
    }
}
#[test]
fn socks5h_and_http_connect_proxies_route_without_local_target_dns() {
    for (scheme, via_env) in [("socks5h", false), ("socks5", false), ("http", false), ("http", true)] {
        let root = tempfile::tempdir().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let proxy = format!("{scheme}://{}", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let mut stream = accept(&listener);
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            if scheme.starts_with("socks5") {
                let mut hello = [0; 2];
                stream.read_exact(&mut hello).unwrap();
                assert_eq!(hello[0], 5);
                let mut methods = vec![0; hello[1] as usize];
                stream.read_exact(&mut methods).unwrap();
                stream.write_all(&[5, 0]).unwrap();
                let mut header = [0; 4];
                stream.read_exact(&mut header).unwrap();
                assert_eq!(&header[..3], &[5, 1, 0]);
                if scheme == "socks5h" {
                    assert_eq!(header[3], 3);
                    let mut length = [0];
                    stream.read_exact(&mut length).unwrap();
                    let mut host = vec![0; length[0] as usize];
                    stream.read_exact(&mut host).unwrap();
                    assert_eq!(host, b"model.invalid");
                } else {
                    assert_eq!(header[3], 1);
                    let mut ip = [0; 4];
                    stream.read_exact(&mut ip).unwrap();
                    assert_eq!(ip, [127, 0, 0, 1]);
                }
                let mut port = [0; 2];
                stream.read_exact(&mut port).unwrap();
                assert_eq!(u16::from_be_bytes(port), 80);
                stream.write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 80]).unwrap();
            } else {
                let mut header = Vec::new();
                while !header.ends_with(b"\r\n\r\n") {
                    let mut b = [0];
                    stream.read_exact(&mut b).unwrap();
                    header.push(b[0]);
                }
                let header = String::from_utf8(header).unwrap();
                if header.starts_with("POST http://model.invalid/chat/completions HTTP/1.1") {
                    let length: usize = header
                        .lines()
                        .find_map(|line| {
                            line.split_once(':')
                                .filter(|(key, _)| key.eq_ignore_ascii_case("content-length"))
                                .map(|(_, value)| value.trim().parse().unwrap())
                        })
                        .unwrap();
                    let mut body = vec![0; length];
                    stream.read_exact(&mut body).unwrap();
                    let body: Value = serde_json::from_slice(&body).unwrap();
                    assert_eq!(body["model"], "fixture");
                    reply(&mut stream, 200, &answer(), "");
                    return;
                }
                assert!(header.starts_with("CONNECT model.invalid:80 HTTP/1.1"), "unexpected proxy request line");
                stream.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").unwrap();
            }
            request(&mut stream);
            reply(&mut stream, 200, &answer(), "");
        });
        let mut config = json!({"network_retries":0});
        config[if via_env { "_test_proxy_env" } else { "proxy" }] = json!(proxy);
        let base = if scheme == "socks5" { "http://127.0.0.1" } else { "http://model.invalid" };
        let output = run(root.path(), base, config);
        server.join().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    }
}
#[test]
fn invalid_proxy_fails_without_sending_the_model_request() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let output = run(
        root.path(),
        &format!("http://{}", listener.local_addr().unwrap()),
        json!({"proxy":"bad-protocol://127.0.0.1:3067"}),
    );
    assert!(!output.status.success());
    listener.set_nonblocking(true).unwrap();
    assert!(listener.accept().is_err());
}
#[test]
fn empty_proxy_overrides_environment() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let proxy = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut stream = accept(&listener);
        request(&mut stream);
        reply(&mut stream, 200, &answer(), "");
    });
    let output =
        run(root.path(), &base, json!({"proxy":"","_test_proxy_env":format!("http://{}",proxy.local_addr().unwrap())}));
    server.join().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    proxy.set_nonblocking(true).unwrap();
    assert!(proxy.accept().is_err());
}
#[test]
fn connection_refused_then_restored_resumes_automatically() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let (retry, observed) = std::sync::mpsc::channel();
    let server = thread::spawn(move || {
        observed
            .recv_timeout(Duration::from_secs(10))
            .expect("client must observe connection refusal before the server starts");
        let listener = TcpListener::bind(address).unwrap();
        let mut stream = accept(&listener);
        request(&mut stream);
        reply(&mut stream, 200, &answer(), "");
    });
    let output = run_observed(root.path(), &format!("http://{address}"), json!({"network_retries":1}), Some(retry));
    server.join().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stderr).contains("network retry 1/1"));
}
#[test]
fn malformed_complete_json_response_is_not_retried_or_executed() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut stream = accept(&listener);
        request(&mut stream);
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\n{").unwrap();
    });
    let output = run(root.path(), &base, json!({}));
    server.join().unwrap();
    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("network retry"));
}
#[cfg(feature = "bundled")]
#[test]
fn bundled_manifest_and_worker_use_the_same_executable() {
    let root = tempfile::tempdir().unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_i"));
    support::isolate(&mut command, root.path());
    let output = command.args(["--runtime", "manifest", "memory"]).output().unwrap();
    assert!(output.status.success());
    let manifest: Value = serde_json::from_slice(&output.stdout).unwrap();
    for entry in manifest["tools"].as_array().unwrap().iter().chain(manifest["hooks"].as_array().unwrap()) {
        assert_eq!(entry["exec"][0], env!("CARGO_BIN_EXE_i"));
        assert_eq!(entry["exec"][1], "--runtime");
        assert_eq!(entry["exec"][2], "memory");
    }
    let mut command = Command::new(env!("CARGO_BIN_EXE_i"));
    support::isolate(&mut command, root.path());
    let mut worker =
        command.args(["--runtime", "memory"]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    for id in [1, 2] {
        writeln!(
            worker.stdin.as_mut().unwrap(),
            "{}",
            json!({"id":id,"event":"before_request","workspace":root.path(),"data":{}})
        )
        .unwrap();
    }
    drop(worker.stdin.take());
    let output = worker.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().lines().count(), 2);
}
