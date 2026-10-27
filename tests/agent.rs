use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

mod approval_cases;
mod cache_cases;
mod compaction_cases;
mod provider_cases;
mod support;
fn workspace_store(root: &Path) -> std::path::PathBuf {
    let mut command = Command::new(plugin_binary());
    support::isolate(&mut command, root);
    let output = command.args(["paths", "-C", root.to_str().unwrap()]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let paths: Value = serde_json::from_slice(&output.stdout).unwrap();
    paths["workspace_data"].as_str().unwrap().into()
}
// A real HTTP fixture exercises the CLI without credentials or external services.
fn server(replies: Vec<(u16, Value)>) -> (String, thread::JoinHandle<Vec<Value>>) {
    server_protocol(replies, false)
}

fn server_protocol(replies: Vec<(u16, Value)>, anthropic: bool) -> (String, thread::JoinHandle<Vec<Value>>) {
    server_route(replies, if anthropic { "messages" } else { "chat/completions" })
}

fn server_route(replies: Vec<(u16, Value)>, route: &'static str) -> (String, thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let mut requests = vec![];
        for (status, reply) in replies {
            let deadline = Instant::now() + Duration::from_secs(15);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "request timed out");
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("{error}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut bytes = Vec::new();
            let mut byte = [0];
            while !bytes.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                bytes.push(byte[0]);
            }
            let headers = String::from_utf8(bytes).unwrap();
            assert!(headers.starts_with(&format!("POST /v1/{route} HTTP/1.1")));
            if route == "messages" {
                assert!(headers.to_lowercase().contains("anthropic-version: 2023-06-01"));
            }
            let length: usize = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length").then(|| value.trim().parse().unwrap())
                })
                .unwrap();
            let mut body = vec![0; length];
            stream.read_exact(&mut body).unwrap();
            requests.push(serde_json::from_slice(&body).unwrap());
            let body = reply.to_string();
            write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        }
        requests
    });
    (base, handle)
}

fn final_reply() -> Value {
    json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":"All done."}}]})
}

fn tool_reply(name: &str, args: Value) -> Value {
    json!({"choices":[{"finish_reason":"tool_calls","message":{"role":"assistant","content":null,
        "tool_calls":[{"id":"call_1","type":"function","function":{"name":name,"arguments":args.to_string()}}]}}]})
}

fn run(dir: &std::path::Path, base: &str, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_i"));
    support::isolate(&mut command, dir);
    command
        .args(["-C", dir.to_str().unwrap(), "--base-url", base, "--model", "test-model"])
        .args(args)
        .env_remove("API_KEY")
        .env("NO_PROXY", "127.0.0.1")
        .env("no_proxy", "127.0.0.1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Keep failing tests bounded even if a regression hangs the agent.
    let mut child = command.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() > deadline {
            child.kill().unwrap();
            panic!("agent timed out");
        }
        thread::sleep(Duration::from_millis(10));
    }
    child.wait_with_output().unwrap()
}

#[test]
fn tool_loop_and_session_resume() {
    let dir = tempfile::tempdir().unwrap();
    let session = dir.path().join("session.json");
    let session = session.to_str().unwrap();
    let (base, server) = server(vec![
        (200, tool_reply("write", json!({"path":"hello.txt","content":"hello"}))),
        (200, final_reply()),
        (200, final_reply()),
    ]);
    let output = run(dir.path(), &base, &["--yolo", "--session", session, "write hello"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(fs::read_to_string(dir.path().join("hello.txt")).unwrap(), "hello");
    assert!(String::from_utf8_lossy(&output.stdout).contains("All done."));
    let output = run(dir.path(), &base, &["--session", session, "summarize"]);
    assert!(output.status.success());
    let requests = server.join().unwrap();
    assert_eq!(requests[0]["model"], "test-model");
    assert_eq!(requests[0]["tools"].as_array().unwrap().len(), 9);
    assert_eq!(requests[1]["messages"][3]["role"], "tool");
    assert_eq!(requests[1]["messages"][3]["tool_call_id"], "call_1");
    assert_eq!(requests[2]["messages"][5]["content"], "summarize");
}

#[test]
fn denied_write_is_returned_to_model() {
    let dir = tempfile::tempdir().unwrap();
    let (base, server) =
        server(vec![(200, tool_reply("write", json!({"path":"denied.txt","content":"no"}))), (200, final_reply())]);
    let output = run(dir.path(), &base, &["try writing"]);
    assert!(output.status.success());
    assert!(!dir.path().join("denied.txt").exists());
    let requests = server.join().unwrap();
    assert!(requests[1]["messages"][3]["content"].as_str().unwrap().contains("user denied"));
}

#[test]
fn native_image_can_be_sent_without_a_caption() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let root = tempfile::tempdir().unwrap();
    let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aN1cAAAAASUVORK5CYII=";
    fs::write(root.path().join("image.png"), STANDARD.decode(png).unwrap()).unwrap();
    let (base, worker) = server(vec![(200, final_reply())]);
    let output = run(root.path(), &base, &["--image", "image.png", ""]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let requests = worker.join().unwrap();
    let content = &requests[0]["messages"][1]["content"];
    assert_eq!(content.as_array().unwrap().len(), 1);
    assert_eq!(content[0]["type"], "image_url");
}

#[test]
fn api_failure_exits_nonzero() {
    let dir = tempfile::tempdir().unwrap();
    let (base, server) = server(vec![(401, json!({"error":{"message":"bad key"}}))]);
    let output = run(dir.path(), &base, &["hello"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("401"));
    server.join().unwrap();
}

#[test]
fn step_limit_preserves_complete_tool_batch() {
    let dir = tempfile::tempdir().unwrap();
    let session = dir.path().join("session.json");
    let (base, server) = server(vec![(200, tool_reply("read", json!({"files":[{"path":"missing"}]})))]);
    let output = run(dir.path(), &base, &["--max-steps", "1", "--session", session.to_str().unwrap(), "inspect"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("step limit"));
    let session: Value = serde_json::from_slice(&fs::read(session).unwrap()).unwrap();
    assert_eq!(session["messages"].as_array().unwrap().last().unwrap()["role"], "tool");
    server.join().unwrap();
}

#[test]
fn configured_images_and_reasoning_round_trip() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let dir = tempfile::tempdir().unwrap();
    let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aN1cAAAAASUVORK5CYII=";
    fs::write(dir.path().join("pixel.png"), STANDARD.decode(png).unwrap()).unwrap();
    let config = dir.path().join("api.json");
    fs::write(&config, json!({"extra_body":{"thinking":{"type":"disabled"}}}).to_string()).unwrap();
    let mut reply = tool_reply("view_image", json!({"path":"pixel.png"}));
    reply["choices"][0]["message"]["reasoning_content"] = json!("fixture reasoning");
    let (base, server) = server(vec![(200, reply), (200, final_reply())]);
    let output = run(dir.path(), &base, &["--config", config.to_str().unwrap(), "--image", "pixel.png", "describe"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let requests = server.join().unwrap();
    assert_eq!(requests[0]["thinking"]["type"], "disabled");
    assert_eq!(requests[0]["max_completion_tokens"], 8192);
    assert!(requests[0].get("max_tokens").is_none());
    assert!(requests[0].get("parallel_tool_calls").is_none());
    let url = format!("data:image/png;base64,{png}");
    assert_eq!(requests[0]["messages"][1]["content"][1]["image_url"]["url"], url);
    assert_eq!(requests[1]["messages"][2]["reasoning_content"], "fixture reasoning");
    assert_eq!(requests[1]["messages"][3]["role"], "tool");
    assert_eq!(requests[1]["messages"][4]["role"], "user");
    assert_eq!(requests[1]["messages"][4]["content"][1]["image_url"]["url"], url);
}

#[test]
fn invalid_image_fails_before_network_request() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("fake.png"), "not an image").unwrap();
    let output = run(dir.path(), "http://127.0.0.1:1", &["--image", "fake.png", "describe"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported image format"));
}

#[test]
fn anthropic_tool_images_and_resume() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("pixel.png"), b"\x89PNG\r\n\x1a\n").unwrap();
    let session = dir.path().join("session.json");
    let session = session.to_str().unwrap();
    let final_response = json!({"type":"message","role":"assistant","stop_reason":"end_turn",
        "content":[{"type":"text","text":"Done"}]});
    let blocks = json!([
        {"type":"thinking","thinking":"inspect","signature":"test-signature"},
        {"type":"tool_use","id":"img1","name":"view_image","input":{"path":"pixel.png"}},
        {"type":"tool_use","id":"read1","name":"read","input":{"path":"missing"}}
    ]);
    let (base, server) = server_protocol(
        vec![
            (200, json!({"type":"message","role":"assistant","stop_reason":"tool_use","content":blocks})),
            (200, final_response.clone()),
            (200, final_response),
        ],
        true,
    );
    let output =
        run(dir.path(), &base, &["--protocol", "anthropic", "--image", "pixel.png", "--session", session, "inspect"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let output = run(dir.path(), &base, &["--protocol", "anthropic", "--session", session, "continue"]);
    assert!(output.status.success());
    let requests = server.join().unwrap();
    assert!(requests[0]["system"].is_array());
    assert_eq!(requests[0]["tools"][0]["input_schema"]["type"], "object");
    assert_eq!(requests[0]["messages"][0]["content"][1]["source"]["media_type"], "image/png");
    assert_eq!(requests[1]["messages"][1]["content"], blocks);
    let content = &requests[1]["messages"][2]["content"];
    assert_eq!(content[0]["tool_use_id"], "img1");
    assert_eq!(content[1]["tool_use_id"], "read1");
    assert_eq!(content[1]["is_error"], true);
    assert_eq!(content[3]["type"], "image");
    assert_eq!(requests[2]["messages"][1]["content"][0]["signature"], "test-signature");
}

#[test]
fn plugins_work_on_both_protocols_and_require_approval() {
    for anthropic in [false, true] {
        for allow in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let manifest = dir.path().join("plugins.json");
            fs::write(
                &manifest,
                json!({"tools":[{"name":"echo_args","description":"Echo JSON arguments",
                "parameters":{"type":"object","properties":{"text":{"type":"string"}}},
                "exec":support::fixture("echo-args",json!({}))}]})
                .to_string(),
            )
            .unwrap();
            let argument = "$(touch injected); 'quoted'";
            let replies = if anthropic {
                vec![
                    (
                        200,
                        json!({"type":"message","role":"assistant","stop_reason":"tool_use",
                    "content":[{"type":"tool_use","id":"plugin1","name":"echo_args","input":{"text":argument}}]}),
                    ),
                    (
                        200,
                        json!({"type":"message","role":"assistant","stop_reason":"end_turn",
                        "content":[{"type":"text","text":"done"}]}),
                    ),
                ]
            } else {
                vec![(200, tool_reply("echo_args", json!({"text":argument}))), (200, final_reply())]
            };
            let (base, server) = server_protocol(replies, anthropic);
            let mut args = vec![
                "--plugins",
                manifest.to_str().unwrap(),
                "--protocol",
                if anthropic { "anthropic" } else { "openai" },
            ];
            if allow {
                args.push("--yolo");
            }
            args.push("use plugin");
            let output = run(dir.path(), &base, &args);
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
            assert!(!dir.path().join("injected").exists());
            let requests = server.join().unwrap();
            assert_eq!(requests[0]["tools"].as_array().unwrap().len(), 10);
            let result = if anthropic {
                &requests[1]["messages"][2]["content"][0]["content"]
            } else {
                &requests[1]["messages"][3]["content"]
            };
            assert!(result.as_str().unwrap().contains(if allow { argument } else { "user denied" }));
        }
    }
}

#[test]
fn config_drives_protocol_and_cli_overrides_connection_and_model() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("api.json");
    fs::write(
        &config,
        json!({"protocol":"anthropic", "base_url":"http://127.0.0.1:1",
        "model":"config-model", "api_key_env":"I_TEST_UNSET_KEY", "max_tokens":123,
        "extra_body":{"temperature":0.3}})
        .to_string(),
    )
    .unwrap();
    let reply = json!({"type":"message","role":"assistant","stop_reason":"end_turn",
        "content":[{"type":"text","text":"done"}]});
    let (base, server) = server_protocol(vec![(200, reply)], true);
    let output = run(dir.path(), &base, &["--config", config.to_str().unwrap(), "hello"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let requests = server.join().unwrap();
    assert_eq!(requests[0]["model"], "test-model");
    assert_eq!(requests[0]["max_tokens"], 123);
    assert_eq!(requests[0]["temperature"], 0.3);
}

fn responses_manifest() -> String {
    format!("{}/plugins/responses.json", env!("CARGO_MANIFEST_DIR"))
}

fn plugin_binary() -> String {
    format!("{}/target/debug/i-plugins{}", env!("CARGO_MANIFEST_DIR"), std::env::consts::EXE_SUFFIX)
}

fn budget_hooks() -> Value {
    json!([{"events":["context_budget","before_request"],"exec":[plugin_binary(),"budget"]},
        {"events":["compact_plan","before_compact","after_compact"],"exec":[plugin_binary(),"compact"]}])
}

fn test_budget(images: &str) -> Value {
    // Trigger early while leaving enough room for the serialized summary input and output reservation.
    json!({"context_window":40000,"compact_ratio":0.25,"safety_ratio":0.02,"image_tokens":1000,"images":images})
}

fn responses_hooks() -> Value {
    json!([{"events":["before_request","after_response"],"exec":[plugin_binary(),"responses"]}])
}

#[test]
fn responses_images_plugins_and_session_replay() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("pixel.png"), b"\x89PNG\r\n\x1a\n").unwrap();
    let session = dir.path().join("session.json");
    let plugins = dir.path().join("plugins.json");
    fs::write(
        &plugins,
        json!({"tools":[{"name":"echo_args","description":"test",
        "parameters":{"type":"object","properties":{}},"exec":support::fixture("constant-tool",json!({"output":"plugin-ok"}))}],"hooks":responses_hooks()})
        .to_string(),
    )
    .unwrap();
    let output = json!([
        {"type":"reasoning","id":"rs1","summary":[],"encrypted_content":"opaque-test",
            "content":[{"type":"reasoning_text","text":"inspect image"}]},
        {"type":"function_call","id":"fc1","call_id":"call_img","name":"view_image","arguments":"{\"path\":\"pixel.png\"}"},
        {"type":"function_call","id":"fc2","call_id":"call_plugin","name":"echo_args","arguments":"{}"}
    ]);
    let final_reply = json!({"status":"completed","output":[{"type":"message","id":"msg1",
        "role":"assistant","status":"completed","content":[{"type":"output_text","text":"Done","annotations":[]}]}]});
    let (base, server) = server_route(
        vec![(200, json!({"status":"completed","output":output})), (200, final_reply.clone()), (200, final_reply)],
        "responses",
    );
    let session_path = session.to_str().unwrap();
    let plugin_path = plugins.to_str().unwrap();
    let result = run(
        dir.path(),
        &base,
        &["--image", "pixel.png", "--session", session_path, "--plugins", plugin_path, "--yolo", "inspect"],
    );
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let result = run(dir.path(), &base, &["--session", session_path, "--plugins", plugin_path, "continue"]);
    assert!(result.status.success());
    let requests = server.join().unwrap();
    assert_eq!(requests[0]["store"], false);
    assert_eq!(requests[0]["max_output_tokens"], 8192);
    assert!(requests[0].get("messages").is_none());
    assert!(requests[0].get("previous_response_id").is_none());
    assert_eq!(requests[0]["tools"][0]["type"], "function");
    assert_eq!(requests[0]["tools"][0]["strict"], false);
    assert!(requests[0]["tools"].as_array().unwrap().iter().any(|tool| tool["name"] == "echo_args"));
    assert_eq!(requests[0]["input"][1]["content"][1]["type"], "input_image");
    assert!(requests[0]["input"][1]["content"][1]["image_url"].as_str().unwrap().starts_with("data:image/png;base64,"));
    assert_eq!(requests[1]["input"][2], output[0]);
    assert_eq!(requests[1]["input"][5]["type"], "function_call_output");
    assert_eq!(requests[1]["input"][5]["call_id"], "call_img");
    assert!(requests[1]["input"][6]["output"].as_str().unwrap().contains("plugin-ok"));
    assert_eq!(requests[1]["input"][7]["content"][1]["type"], "input_image");
    assert_eq!(requests[2]["input"][2], output[0]);
    assert_eq!(requests[2]["input"].as_array().unwrap().last().unwrap()["content"], "continue");
}

#[test]
fn responses_incomplete_never_executes_tools() {
    let dir = tempfile::tempdir().unwrap();
    let response = json!({"status":"incomplete","incomplete_details":{"reason":"max_output_tokens"},
        "output":[{"type":"function_call","call_id":"call1","name":"write",
            "arguments":"{\"path\":\"bad.txt\",\"content\":\"bad\"}"}]});
    let (base, server) = server_route(vec![(200, response)], "responses");
    let output = run(dir.path(), &base, &["--plugins", &responses_manifest(), "--yolo", "test"]);
    assert!(!output.status.success());
    assert!(!dir.path().join("bad.txt").exists());
    assert!(String::from_utf8_lossy(&output.stderr).contains("max_output_tokens"));
    server.join().unwrap();
}

#[test]
fn openai_medium_reasoning_examples_match_wire_format() {
    for (source, responses) in [
        (include_str!("../examples/openai.chat.medium.json"), false),
        (include_str!("../examples/openai.responses.medium.json"), true),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("api.json");
        let mut value: Value = serde_json::from_str(source).unwrap();
        value["api_key_env"] = json!("I_TEST_UNSET_KEY");
        value["plugins"] = json!(format!(
            "{}/plugins/{}",
            env!("CARGO_MANIFEST_DIR"),
            if responses { "responses-compact.json" } else { "compact.json" }
        ));
        fs::write(&config, value.to_string()).unwrap();
        let reply = if responses {
            json!({"status":"completed","output":[{"type":"message","role":"assistant",
            "content":[{"type":"output_text","text":"done"}]}]})
        } else {
            final_reply()
        };
        let (base, server) = server_route(vec![(200, reply)], if responses { "responses" } else { "chat/completions" });
        let output = run(dir.path(), &base, &["--config", config.to_str().unwrap(), "reason carefully"]);
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let requests = server.join().unwrap();
        let body = &requests[0];
        assert!(body.get("max_tokens").is_none());
        if responses {
            assert_eq!(body["reasoning"]["effort"], "medium");
            assert_eq!(body["max_output_tokens"], 25000);
            assert_eq!(body["include"], json!(["reasoning.encrypted_content"]));
            assert!(body.get("reasoning_effort").is_none());
        } else {
            assert_eq!(body["reasoning_effort"], "medium");
            assert_eq!(body["max_completion_tokens"], 25000);
            assert!(body.get("reasoning").is_none());
        }
    }
}

#[test]
fn chat_token_field_is_explicit_and_respects_cli_override() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("api.json");
    fs::write(&config, json!({"max_tokens":10,"use_max_tokens":true}).to_string()).unwrap();
    let (base, server) = server(vec![(200, final_reply())]);
    let output = run(dir.path(), &base, &["--config", config.to_str().unwrap(), "--max-tokens", "456", "test"]);
    assert!(output.status.success());
    let requests = server.join().unwrap();
    assert_eq!(requests[0]["max_tokens"], 456);
    assert!(requests[0].get("max_completion_tokens").is_none());
}

fn manage(dir: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_i"))
        .arg("-C")
        .arg(dir)
        .args(args)
        .env_remove("API_MODEL")
        .env_remove("API_BASE_URL")
        .env_remove("API_KEY")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn named_sessions_create_resume_list_show_and_delete() {
    let dir = tempfile::tempdir().unwrap();
    let output = manage(dir.path(), &["--sessions", "list"]);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!dir.path().join(".i").exists());
    let (base, server) = server(vec![(200, final_reply()), (200, final_reply())]);
    for task in ["first task", "second task"] {
        let output = run(dir.path(), &base, &["--session-name", "fix-bug", task]);
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    }
    let requests = server.join().unwrap();
    assert_eq!(requests[1]["messages"][1]["content"], "first task");
    assert_eq!(requests[1]["messages"][3]["content"], "second task");
    let path = dir.path().join(".i/sessions/fix-bug.json");
    assert!(path.is_file());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    }
    let output = manage(dir.path(), &["--sessions", "list"]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("fix-bug.json"));
    let output = manage(dir.path(), &["--sessions", "show", "--session-name", "fix-bug"]);
    assert!(output.status.success());
    let history: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(history[1]["content"], "first task");
    assert_eq!(history[3]["content"], "second task");
    let output = manage(dir.path(), &["--sessions", "delete", "--session-name", "fix-bug"]);
    assert!(!output.status.success());
    assert!(path.exists());
    let output =
        manage(dir.path(), &["--sessions", "delete", "--session-name", "fix-bug", "--dangerously-skip-permissions"]);
    assert!(output.status.success());
    assert!(!path.exists());
}

#[test]
fn session_management_rejects_invalid_names_conflicts_and_missing_files() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["../escape", "/absolute", "has.dot", "", "has space"] {
        let output = manage(dir.path(), &["--sessions", "show", "--session-name", name]);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("invalid session name"));
    }
    for args in [
        vec!["--sessions", "show"],
        vec!["--sessions", "delete", "--session-name", "missing", "--yolo"],
        vec!["--sessions", "show", "--session-name", "missing"],
        vec!["--sessions", "list", "--session-name", "unwanted"],
        vec!["--sessions", "list", "not a task"],
        vec!["--session-name", "name", "--session", "file.json"],
    ] {
        assert!(!manage(dir.path(), &args).status.success(), "accepted {args:?}");
    }
}

#[cfg(unix)]
#[test]
fn managed_sessions_reject_symlink_escape() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join(".i")).unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.path().join(".i/sessions")).unwrap();
    assert!(!manage(dir.path(), &["--sessions", "list"]).status.success());
    assert!(!manage(dir.path(), &["--session-name", "escape", "hello"]).status.success());
}

fn seed_history(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("session.json");
    let messages = json!([
        {"role":"system","content":"original system"},
        {"role":"user","content":[{"type":"text","text":"old goal"},{"type":"image_url","image_url":{"url":format!("data:image/png;base64,{}", "x".repeat(5000))}}]},
        {"role":"assistant","content":null,"tool_calls":[{"id":"old_call","type":"function","function":{"name":"read","arguments":"{\"path\":\"old\"}"}}]},
        {"role":"tool","tool_call_id":"old_call","content":"old output ".repeat(1000)},
        {"role":"assistant","content":"old result"},
        {"role":"user","content":"recent goal"},
        {"role":"assistant","content":"recent result"}
    ]);
    fs::write(
        &path,
        json!({"version":1,"workspace":dunce::canonicalize(dir).unwrap(),"messages":messages}).to_string(),
    )
    .unwrap();
    path
}

fn answer(protocol: &str, text: &str) -> Value {
    match protocol {
        "responses" => {
            json!({"status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":text}]}]})
        }
        "anthropic" => {
            json!({"type":"message","role":"assistant","stop_reason":"end_turn","content":[{"type":"text","text":text}]})
        }
        _ => json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":text}}]}),
    }
}

#[test]
fn auto_compaction_with_prompt_hooks_and_backups_on_all_transports() {
    for protocol in ["openai", "anthropic", "responses"] {
        let dir = tempfile::tempdir().unwrap();
        let session = seed_history(dir.path());
        let mut hooks = if protocol == "responses" { responses_hooks() } else { json!([]) };
        hooks.as_array_mut().unwrap().push(
            json!({"events":["before_compact","after_compact"],"exec":support::fixture("compact-prompt",json!({}))}),
        );
        hooks.as_array_mut().unwrap().extend(budget_hooks().as_array().unwrap().clone());
        fs::write(dir.path().join("hooks.json"), json!({"tools":[],"hooks":hooks}).to_string()).unwrap();
        let config = dir.path().join("api.json");
        fs::write(
            &config,
            json!({"protocol":if protocol == "responses" {"openai"} else {protocol},
            "plugins":"hooks.json","budget":test_budget("omit")})
            .to_string(),
        )
        .unwrap();
        let route = match protocol {
            "responses" => "responses",
            "anthropic" => "messages",
            _ => "chat/completions",
        };
        let (base, server) =
            server_route(vec![(200, answer(protocol, "SHORT SUMMARY")), (200, answer(protocol, "done"))], route);
        let output = run(
            dir.path(),
            &base,
            &["--config", config.to_str().unwrap(), "--session", session.to_str().unwrap(), "current task"],
        );
        assert!(output.status.success(), "{protocol}: {}", String::from_utf8_lossy(&output.stderr));
        assert!(String::from_utf8_lossy(&output.stderr).contains("auto-compact"));
        let requests = server.join().unwrap();
        assert!(requests[0].get("tools").is_none(), "{protocol}: {}", requests[0]);
        let summary_request = requests[0].to_string();
        assert!(summary_request.contains("CUSTOM COMPACTION PROMPT"));
        assert!(summary_request.contains("handled separately: omit"));
        assert!(!summary_request.contains("data:image"));
        assert!(requests[1].to_string().contains("SHORT SUMMARY [hook]"));
        assert!(requests[1].to_string().contains("current task"));
        let compacted: Value = serde_json::from_slice(&fs::read(&session).unwrap()).unwrap();
        assert!(compacted["messages"][1]["content"].as_str().unwrap().contains("SHORT SUMMARY [hook]"));
        let backup: Value =
            serde_json::from_slice(&fs::read(session.with_extension("precompact.json")).unwrap()).unwrap();
        assert_eq!(backup["messages"][3]["tool_call_id"], "old_call");
        assert!(backup.to_string().contains("data:image/png"));
    }
}

#[test]
fn failed_compaction_keeps_full_history_and_can_be_disabled() {
    for disabled in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let session = seed_history(dir.path());
        let config = dir.path().join("api.json");
        let mut policy = test_budget("omit");
        policy["enabled"] = json!(!disabled);
        fs::write(
            &config,
            json!({"network_retries":0,"budget":policy,"plugins":format!("{}/plugins/compact.json",env!("CARGO_MANIFEST_DIR"))}).to_string(),
        )
        .unwrap();
        let response = if disabled { (200, final_reply()) } else { (500, json!({"error":"summary failed"})) };
        let (base, server) = server(vec![response]);
        let output = run(
            dir.path(),
            &base,
            &["--config", config.to_str().unwrap(), "--session", session.to_str().unwrap(), "continue"],
        );
        assert_eq!(output.status.success(), disabled);
        assert!(!session.with_extension("precompact.json").exists());
        let saved: Value = serde_json::from_slice(&fs::read(&session).unwrap()).unwrap();
        assert_eq!(saved["messages"][3]["tool_call_id"], "old_call");
        assert!(saved.to_string().contains("data:image/png"));
        let requests = server.join().unwrap();
        assert_eq!(requests[0].get("tools").is_some(), disabled);
    }
}

#[test]
fn lifecycle_hooks_modify_inputs_tools_and_results_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = dir.path().join("hooks.json");
    fs::write(
        &manifest,
        json!({"tools":[],"hooks":[{"events":["*"],"exec":support::fixture("lifecycle",json!({}))}]}).to_string(),
    )
    .unwrap();
    let (base, server) =
        server(vec![(200, tool_reply("write", json!({"path":"wrong.txt","content":"wrong"}))), (200, final_reply())]);
    let output = run(dir.path(), &base, &["--plugins", manifest.to_str().unwrap(), "--yolo", "start"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(fs::read_to_string(dir.path().join("created.txt")).unwrap(), "HOOK CONTENT");
    assert!(!dir.path().join("wrong.txt").exists());
    assert!(String::from_utf8_lossy(&output.stdout).contains("HOOK FINAL"));
    let requests = server.join().unwrap();
    assert!(requests[0]["messages"][0]["content"].as_str().unwrap().contains("CUSTOM SYSTEM"));
    assert_eq!(requests[0]["messages"][1]["content"], "start [edited input]");
    assert_eq!(requests[0]["temperature"], 0.25);
    assert!(requests[1]["messages"][3]["content"].as_str().unwrap().contains("[edited result]"));
    let events = fs::read_to_string(dir.path().join("events.log")).unwrap();
    for event in [
        "session_load",
        "system_prompt",
        "user_input",
        "before_request",
        "after_response",
        "before_tool",
        "after_tool",
        "before_save",
        "after_save",
        "finish",
    ] {
        assert!(events.lines().any(|line| line == event), "missing {event}");
    }
    assert!(events.find("before_tool").unwrap() < events.find("after_tool").unwrap());
}

#[test]
fn memory_tools_persist_and_inject_only_when_loaded() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = format!("{}/plugins/memory.json", env!("CARGO_MANIFEST_DIR"));
    let (base, server) = server(vec![
        (200, tool_reply("memory_set", json!({"key":"language","value":"Prefer Rust"}))),
        (200, final_reply()),
        (200, final_reply()),
        (200, final_reply()),
    ]);
    let output = run(dir.path(), &base, &["--plugins", &manifest, "--yolo", "remember language"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let memory: Value =
        serde_json::from_slice(&fs::read(support::store(dir.path()).join("memory.json")).unwrap()).unwrap();
    assert_eq!(memory["language"], "Prefer Rust");
    assert!(run(dir.path(), &base, &["--plugins", &manifest, "new task"]).status.success());
    assert!(run(dir.path(), &base, &["no memory plugin"]).status.success());
    let requests = server.join().unwrap();
    assert_eq!(requests[0]["messages"][0], requests[2]["messages"][0]);
    assert!(requests[2]["messages"][2]["content"].as_str().unwrap().contains("Prefer Rust"));
    assert_eq!(&requests[1]["messages"].as_array().unwrap()[..2], requests[0]["messages"].as_array().unwrap());
    assert!(!requests[3]["messages"].to_string().contains("Prefer Rust"));
    assert_eq!(requests[0]["tools"].as_array().unwrap().len(), 12);
}

#[test]
fn malicious_history_and_cross_origin_hook_changes_fail_closed() {
    for (event, mode, expected) in [
        ("before_save", "orphan-history", "orphan tool result"),
        ("before_request", "cross-origin", "cannot change API origin"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let manifest = dir.path().join("hooks.json");
        fs::write(
            &manifest,
            json!({"tools":[],"hooks":[{"events":[event],"exec":support::fixture(mode,json!({}))}]}).to_string(),
        )
        .unwrap();
        let output = run(dir.path(), "http://127.0.0.1:1", &["--plugins", manifest.to_str().unwrap(), "test"]);
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn full_manifest_composes_prompts_memory_and_responses() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(workspace_store(dir.path()).join("memory.json"), r#"{"preference":"Prefer offline tests"}"#).unwrap();
    let manifest = format!("{}/plugins/full.json", env!("CARGO_MANIFEST_DIR"));
    let (base, server) = server_route(vec![(200, answer("responses", "done"))], "responses");
    let config = dir.path().join("api.json");
    fs::write(&config, json!({"budget":test_budget("keep")}).to_string()).unwrap();
    let output = run(
        dir.path(),
        &base,
        &["--config", config.to_str().unwrap(), "--plugins", &manifest, "--session-name", "combined", "check project"],
    );
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let requests = server.join().unwrap();
    let system = requests[0]["input"][0]["content"].as_str().unwrap();
    assert!(system.contains("careful coding agent"));
    assert!(!system.contains("Prefer offline tests"));
    assert!(requests[0]["input"][2]["content"].to_string().contains("Prefer offline tests"));
    let stats: Value =
        serde_json::from_slice(&fs::read(support::store(dir.path()).join("metrics/cache-stats.json")).unwrap())
            .unwrap();
    assert_eq!(stats["totals"]["observed_responses"], 1);
    assert_eq!(stats["totals"]["unknown_usage_responses"], 1);
    assert!(!system.contains("{{workspace}}"));
    assert_eq!(requests[0]["tools"].as_array().unwrap().len(), 17);
    assert!(dir.path().join(".i/sessions/combined.json").exists());
}

#[test]
fn compaction_keeps_describes_or_omits_images_without_changing_files() {
    for mode in ["keep", "describe", "omit"] {
        let dir = tempfile::tempdir().unwrap();
        let session = seed_history(dir.path());
        let image = dir.path().join("original.png");
        fs::write(&image, b"unchanged original image").unwrap();
        let config = dir.path().join("api.json");
        fs::write(
            &config,
            json!({"budget":test_budget(mode),"plugins":format!("{}/plugins/compact.json",env!("CARGO_MANIFEST_DIR"))})
                .to_string(),
        )
        .unwrap();
        let (base, server) = server(vec![(200, answer("openai", "Short visual/task summary")), (200, final_reply())]);
        let output = run(
            dir.path(),
            &base,
            &["--config", config.to_str().unwrap(), "--session", session.to_str().unwrap(), "continue"],
        );
        assert!(output.status.success(), "{mode}: {}", String::from_utf8_lossy(&output.stderr));
        let requests = server.join().unwrap();
        let summary_content = &requests[0]["messages"][1]["content"];
        if mode == "describe" {
            assert_eq!(summary_content[1]["type"], "image_url");
            assert!(!summary_content[0]["text"].as_str().unwrap().contains("data:image"));
        } else {
            assert!(summary_content.is_string());
            assert!(!summary_content.as_str().unwrap().contains("data:image"));
        }
        assert_eq!(requests[1]["messages"][1]["content"].is_array(), mode == "keep");
        assert_eq!(fs::read(&image).unwrap(), b"unchanged original image");
        assert!(fs::read_to_string(session.with_extension("precompact.json")).unwrap().contains("data:image/png"));
    }
}

#[test]
fn missing_budget_metadata_fails_before_http_and_io_limit_is_configurable() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("api.json");
    fs::write(&config, json!({"plugins":format!("{}/plugins/budget.json",env!("CARGO_MANIFEST_DIR"))}).to_string())
        .unwrap();
    let output = run(dir.path(), "http://127.0.0.1:1", &["--config", config.to_str().unwrap(), "test"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("context_window"));
    fs::write(dir.path().join("long.txt"), "long content ".repeat(5000)).unwrap();
    fs::write(&config, json!({"io_limit_bytes":1024}).to_string()).unwrap();
    let (base, server) =
        server(vec![(200, tool_reply("read", json!({"files":[{"path":"long.txt"}]}))), (200, final_reply())]);
    assert!(run(dir.path(), &base, &["--config", config.to_str().unwrap(), "read"]).status.success());
    let requests = server.join().unwrap();
    let result = requests[1]["messages"][3]["content"].as_str().unwrap();
    assert!(result.len() <= 1024 && result.contains("truncated"));
}

#[test]
fn console_accepts_steering_during_tools_and_queues_followups() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = dir.path().join("plugins.json");
    fs::write(&manifest, json!({"tools":[{"name":"slow","description":"slow fixture","parameters":{"type":"object"},"exec":support::fixture("slow-tool",json!({}))}],"hooks":[]}).to_string()).unwrap();
    let (base, server) = server(vec![(200, tool_reply("slow", json!({}))), (200, final_reply()), (200, final_reply())]);
    let mut command = Command::new(plugin_binary());
    support::isolate(&mut command, dir.path());
    let mut child = command
        .arg("console")
        .args([
            "--bin",
            env!("CARGO_BIN_EXE_i"),
            "--task",
            "initial task",
            "--base-url",
            &base,
            "--model",
            "test-model",
            "--session-name",
            "interactive",
            "--plugins",
            manifest.to_str().unwrap(),
            "--yolo",
            "-C",
            dir.path().to_str().unwrap(),
        ])
        .env_remove("API_KEY")
        .env("NO_PROXY", "127.0.0.1")
        .env("no_proxy", "127.0.0.1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    while !dir.path().join("started").exists() {
        assert!(Instant::now() < deadline, "console never started tool");
        if child.try_wait().unwrap().is_some() {
            let output = child.wait_with_output().unwrap();
            panic!("console exited: {}", String::from_utf8_lossy(&output.stderr));
        }
        thread::sleep(Duration::from_millis(10));
    }
    child.stdin.take().unwrap().write_all(b"/steer change direction\n/queue next task\n").unwrap();
    while child.try_wait().unwrap().is_none() {
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("console timed out");
        }
        thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let requests = server.join().unwrap();
    assert_eq!(requests[1]["messages"][3]["role"], "tool");
    assert_eq!(requests[1]["messages"][4]["content"], "change direction");
    assert_eq!(requests[2]["messages"].as_array().unwrap().last().unwrap()["content"], "next task");
    let session: Value =
        serde_json::from_slice(&fs::read(support::store(dir.path()).join("sessions/interactive.json")).unwrap())
            .unwrap();
    assert_eq!(
        session["messages"].as_array().unwrap().iter().filter(|m| m["content"] == "change direction").count(),
        1
    );
}

#[test]
fn built_in_search_returns_paths_and_lines_without_a_plugin() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("main.rs"), "fn main() {}\n// find needle here\n").unwrap();
    let (base, server) =
        server(vec![(200, tool_reply("search", json!({"pattern":"needle","glob":"*.rs"}))), (200, final_reply())]);
    let output = run(dir.path(), &base, &["search project"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let requests = server.join().unwrap();
    let result: Value = serde_json::from_str(requests[1]["messages"][3]["content"].as_str().unwrap()).unwrap();
    assert_eq!(result["results"][0]["path"], "main.rs");
    assert_eq!(result["results"][0]["line"], 2);
}

#[test]
fn skills_are_discovered_loaded_on_demand_and_refreshed_on_resume() {
    let dir = tempfile::tempdir().unwrap();
    let skill = dir.path().join(".agents/skills/checks");
    fs::create_dir_all(&skill).unwrap();
    let page = skill.join("SKILL.md");
    fs::write(&page, "---\nname: checks\ndescription: first description\n---\nBODY_ONLY_ON_DEMAND\nDo not execute scripts without approval.\n").unwrap();
    fs::write(skill.join("script.sh"), "touch SHOULD_NOT_EXIST").unwrap();
    let manifest = format!("{}/plugins/skills.json", env!("CARGO_MANIFEST_DIR"));
    let (base, worker) = server(vec![(200, tool_reply("skill_load", json!({"name":"checks"}))), (200, final_reply())]);
    let result =
        run(dir.path(), &base, &["--plugins", &manifest, "--session-name", "skills", "--yolo", "load relevant skill"]);
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let requests = worker.join().unwrap();
    let original_system = requests[0]["messages"][0].clone();
    let original_prefix = requests[1]["messages"].as_array().unwrap().clone();
    let reference = requests[0]["messages"][2]["content"].as_str().unwrap();
    assert!(reference.contains("first description") && !reference.contains("BODY_ONLY_ON_DEMAND"));
    assert!(!original_system.to_string().contains("first description"));
    assert!(requests[1]["messages"][4]["content"].as_str().unwrap().contains("BODY_ONLY_ON_DEMAND"));
    assert!(!dir.path().join("SHOULD_NOT_EXIST").exists());
    fs::write(&page, "---\nname: checks\ndescription: updated description\n---\nNew body\n").unwrap();
    let (base, worker) = server(vec![(200, final_reply())]);
    assert!(run(dir.path(), &base, &["--plugins", &manifest, "--session-name", "skills", "continue"]).status.success());
    let requests = worker.join().unwrap();
    assert_eq!(requests[0]["messages"][0], original_system);
    let messages = requests[0]["messages"].as_array().unwrap();
    assert_eq!(&messages[..original_prefix.len()], original_prefix.as_slice());
    let update = messages.last().unwrap()["content"].as_str().unwrap();
    assert!(update.contains("updated description") && update.contains("Kind: delta"));
    assert!(!update.contains("first description"));
}

#[test]
fn over_budget_summary_is_rejected_before_http() {
    let dir = tempfile::tempdir().unwrap();
    let session = seed_history(dir.path());
    let mut saved: Value = serde_json::from_slice(&fs::read(&session).unwrap()).unwrap();
    saved["messages"][1]["content"] = json!(" xqz_".repeat(3000));
    fs::write(&session, saved.to_string()).unwrap();
    let config = dir.path().join("api.json");
    fs::write(
        &config,
        json!({"max_tokens":100,"plugins":format!("{}/plugins/compact.json",env!("CARGO_MANIFEST_DIR")),
        "budget":{"context_window":5000,"images":"omit"}})
        .to_string(),
    )
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let output = run(
        dir.path(),
        &format!("http://{}/v1", listener.local_addr().unwrap()),
        &["--config", config.to_str().unwrap(), "--session", session.to_str().unwrap(), "continue"],
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("context budget exhausted"));
    assert!(matches!(listener.accept(),Err(e) if e.kind()==std::io::ErrorKind::WouldBlock));
    assert!(!session.with_extension("precompact.json").exists());
    let after: Value = serde_json::from_slice(&fs::read(&session).unwrap()).unwrap();
    assert_eq!(after["messages"][1], saved["messages"][1]);
}

#[test]
fn cache_usage_target_does_not_hide_cold_requests() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("file.txt"), "small new result").unwrap();
    let mut first = tool_reply("read", json!({"files":[{"path":"file.txt"}]}));
    first["usage"] = json!({"prompt_tokens":100000,"prompt_tokens_details":{"cached_tokens":0}});
    let mut second = final_reply();
    second["usage"] = json!({"prompt_tokens":101000,"prompt_tokens_details":{"cached_tokens":100000}});
    let (base, worker) = server(vec![(200, first), (200, second)]);
    let manifest = format!("{}/plugins/cache.json", env!("CARGO_MANIFEST_DIR"));
    let output = run(dir.path(), &base, &["--plugins", &manifest, "read file"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let requests = worker.join().unwrap();
    assert_eq!(&requests[1]["messages"].as_array().unwrap()[..2], requests[0]["messages"].as_array().unwrap());
    assert!(!requests[1].to_string().contains("_cache_session"));
    let mut command = Command::new(plugin_binary());
    support::isolate(&mut command, dir.path());
    let report = command.args(["cache-report", "-C", dir.path().to_str().unwrap()]).output().unwrap();
    assert!(report.status.success());
    let report: Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(report["phases"]["append"]["target_met"], true);
    assert_eq!(report["totals"]["target_met"], false);
    assert_eq!(report["totals"]["input_tokens"], 201000);
    assert_eq!(report["totals"]["cache_read_tokens"], 100000);
}

#[test]
fn compaction_restores_reference_snapshots_before_request_and_budget_validation() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(workspace_store(dir.path()).join("memory.json"), r#"{"preference":"KEEP_THIS_CURRENT_MEMORY"}"#).unwrap();
    let session = seed_history(dir.path());
    let mut saved: Value = serde_json::from_slice(&fs::read(&session).unwrap()).unwrap();
    saved["messages"][0]["_context_memory"] =
        json!({"base":"fixture","snapshot":{"preference":"KEEP_THIS_CURRENT_MEMORY"}});
    saved["messages"].as_array_mut().unwrap().insert(
        1,
        json!({"role":"user","content":"old memory snapshot",
        "_context_source":"memory","_context_base":"fixture","_context_full":true}),
    );
    fs::write(&session, saved.to_string()).unwrap();
    let mut hooks = budget_hooks();
    hooks
        .as_array_mut()
        .unwrap()
        .push(json!({"events":["before_turn","before_compact_commit"],"exec":[plugin_binary(),"memory"]}));
    fs::write(dir.path().join("hooks.json"), json!({"tools":[],"hooks":hooks}).to_string()).unwrap();
    let config = dir.path().join("api.json");
    fs::write(&config, json!({"budget":test_budget("omit"),"plugins":"hooks.json"}).to_string()).unwrap();
    let (base, worker) =
        server(vec![(200, answer("openai", "Summary without the reference store")), (200, final_reply())]);
    let output = run(
        dir.path(),
        &base,
        &["--config", config.to_str().unwrap(), "--session", session.to_str().unwrap(), "continue"],
    );
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let requests = worker.join().unwrap();
    let restored = requests[1]["messages"].as_array().unwrap().last().unwrap()["content"].as_str().unwrap();
    assert!(restored.contains("KEEP_THIS_CURRENT_MEMORY") && restored.contains("Kind: snapshot"));
}

#[test]
fn global_config_and_plugin_settings_are_resolved_without_changing_model_prefix_for_branding() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join(".app/config");
    fs::create_dir_all(&config).unwrap();
    fs::write(config.join("config.json"),json!({"model":"home-model","base_url":"http://unused.invalid", "plugins":"desktop.json","extra_body":{"temperature":0.25}}).to_string()).unwrap();
    fs::write(
        config.join("desktop.json"),
        json!({"tools":[],"hooks":[{"events":["system_prompt"],"exec":[plugin_binary(),"prompts"]}]}).to_string(),
    )
    .unwrap();
    fs::write(config.join("prompts.json"), r#"{"system":"system.txt"}"#).unwrap();
    fs::write(config.join("system.txt"), "GLOBAL SYSTEM {{workspace}}").unwrap();
    let (base, worker) = server(vec![(200, final_reply())]);
    let output = run(dir.path(), &base, &["hello"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let requests = worker.join().unwrap();
    assert_eq!(requests[0]["model"], "test-model"); // CLI wins over defaults.
    assert_eq!(requests[0]["temperature"], 0.25);
    assert!(requests[0]["messages"][0]["content"].as_str().unwrap().starts_with("GLOBAL SYSTEM"));
    assert!(!requests[0].to_string().contains("lninl"));
}

#[test]
fn smoke_profile_removes_tools_caps_cost_and_rejects_unexpected_calls() {
    let root = tempfile::tempdir().unwrap();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut config: Value =
        serde_json::from_slice(&fs::read(repo.join("examples/deepseek.smoke.json")).unwrap()).unwrap();
    config["plugins"] = json!(repo.join("plugins/smoke.json"));
    let file = root.path().join("api.json");
    fs::write(&file, config.to_string()).unwrap();
    let (base, worker) = server(vec![(200, final_reply())]);
    assert!(
        run(root.path(), &base, &["--config", file.to_str().unwrap(), "--max-tokens", "200", "say OK"])
            .status
            .success()
    );
    let requests = worker.join().unwrap();
    assert_eq!(requests[0]["max_tokens"], 32);
    assert!(requests[0].get("tools").is_none() && requests[0].get("tool_choice").is_none());
    assert_eq!(requests[0]["thinking"]["type"], "disabled");
    let (base, worker) = server(vec![(200, tool_reply("write", json!({"path":"forbidden","content":"bad"})))]);
    let result = run(root.path(), &base, &["--config", file.to_str().unwrap(), "--yolo", "say OK"]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("smoke check forbids tools"));
    assert!(!root.path().join("forbidden").exists());
    worker.join().unwrap();
}
#[test]
fn external_tool_images_share_native_result_and_approval_pipeline() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = dir.path().join("images.json");
    fs::write(&manifest,json!({"tools":[{"name":"picture","description":"fixture","parameters":{"type":"object"},"exec":support::fixture("picture-tool",json!({}))}]}).to_string()).unwrap();
    let (base, worker) = server(vec![(200, tool_reply("picture", json!({}))), (200, final_reply())]);
    let result = run(dir.path(), &base, &["--plugins", manifest.to_str().unwrap(), "--yolo", "show picture"]);
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let requests = worker.join().unwrap();
    assert_eq!(requests[1]["messages"][3]["role"], "tool");
    assert_eq!(requests[1]["messages"][3]["content"], "external picture");
    assert_eq!(requests[1]["messages"][4]["content"][1]["type"], "image_url");
    let (base, worker) = server(vec![(200, tool_reply("picture", json!({}))), (200, final_reply())]);
    assert!(run(dir.path(), &base, &["--plugins", manifest.to_str().unwrap(), "deny picture"]).status.success());
    let requests = worker.join().unwrap();
    assert!(requests[1]["messages"][3]["content"].as_str().unwrap().contains("user denied"));
    assert_eq!(requests[1]["messages"].as_array().unwrap().len(), 4);
}

#[test]
fn api_response_body_is_bounded_for_success_and_error_statuses() {
    for status in [200, 500] {
        let root = tempfile::tempdir().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}/v1", listener.local_addr().unwrap());
        let worker = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(15);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline);
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => panic!("{e}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            stream.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut header = Vec::new();
            let mut byte = [0];
            while !header.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
            }
            let header = String::from_utf8(header).unwrap();
            let length: usize = header
                .lines()
                .find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case("content-length").then(|| value.trim().parse().unwrap())
                })
                .unwrap();
            stream.read_exact(&mut vec![0; length]).unwrap();
            write!(
                stream,
                "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                49 * 1024 * 1024
            )
            .unwrap();
            for _ in 0..49 * 16 {
                if stream.write_all(&[b'x'; 65536]).is_err() {
                    break;
                }
            }
        });
        let result = run(root.path(), &base, &["test limit"]);
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("API response exceeds 48 MiB"),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        worker.join().unwrap();
    }
}

#[test]
#[ignore = "Legacy 1024-line scope superseded by the approved shared-core/WASM design; see docs/wasm.md"]
fn all_core_production_rust_stays_under_1024_lines() {
    fn count(path: &Path) -> usize {
        if !path.exists() {
            return 0;
        }
        if path.is_dir() {
            return fs::read_dir(path).unwrap().map(|entry| count(&entry.unwrap().path())).sum();
        }
        if path.extension().is_some_and(|ext| ext == "rs") {
            return fs::read_to_string(path).unwrap().lines().count();
        }
        0
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    // Only this explicit, cfg(test)-only unit-test file is exempt, not arbitrary
    // files named tests.rs in subdirectories or production helper modules.
    let lines = count(&root.join("src")) - count(&root.join("src/tests.rs")) + count(&root.join("build.rs"));
    assert!(lines <= 1024, "core production Rust totals {lines} lines (limit 1024)");
}
