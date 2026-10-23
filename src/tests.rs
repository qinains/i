use super::*;
use std::fs;
use tempfile::TempDir;
#[test]
fn auto_approval_flag_names_replace_yes() {
    for flag in ["--yolo", "--dangerously-skip-permissions"] {
        assert!(Args::try_parse_from(["i", flag, "test"]).unwrap().yes);
    }
    assert!(Args::try_parse_from(["i", "--yes", "test"]).is_err());
    assert!(!Args::try_parse_from(["i", "test"]).unwrap().yes);
}
#[path = "../tests/support/mod.rs"]
mod support;
fn rpc(exec: &Value, input: &Value, dir: &Path, limit: usize) -> Result<Value> {
    let home = tempfile::tempdir()?;
    let mut input = input.clone();
    input["paths"] = json!({"config_dir":home.path().join("config"),"data_dir":home.path().join("data"),
        "cache_dir":home.path().join("cache"),"runtime_dir":home.path().join("runtime")});
    Worker::start(exec, dir, limit)?.call(input, Duration::from_secs(3))
}

#[derive(Clone, Copy)]
enum Protocol {
    Openai,
    Anthropic,
}
fn provider_fixture(protocol: Protocol, event: &str, mut data: Value) -> Result<Value> {
    data["version"] = json!(1);
    data["protocol"] = json!(match protocol {
        Protocol::Openai => "openai",
        Protocol::Anthropic => "anthropic",
    });
    let binary = format!("{}/target/debug/i-plugins{}", env!("CARGO_MANIFEST_DIR"), env::consts::EXE_SUFFIX);
    rpc(&json!([binary, "provider"]), &json!({"event":event,"data":data}), &env::current_dir()?, 24000)
}
fn prepared(protocol: Protocol, base: &str, model: &str, history: &[Value], max_tokens: usize) -> Result<Value> {
    provider_fixture(
        protocol,
        "provider_prepare",
        json!({"base_url":base,"model":model,"history":history,
        "max_tokens":max_tokens,"tools":tools(),"plugin_tools":[],"extra_body":{}}),
    )
}
fn endpoint(base: &str, protocol: Protocol) -> String {
    prepared(protocol, base, "test", &[], 1).unwrap()["url"].as_str().unwrap().into()
}
fn request_body(protocol: Protocol, model: &str, history: &[Value], max_tokens: usize) -> Result<Value> {
    Ok(prepared(protocol, "https://provider.invalid", model, history, max_tokens)?["body"].take())
}
fn normalize_response(protocol: Protocol, response: Value) -> Result<Value> {
    Ok(provider_fixture(protocol, "provider_parse", json!({"response":response}))?["response"].take())
}
fn workspace() -> (TempDir, Workspace<NativePlatform>) {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    let platform = NativePlatform::new(root.clone(), readline::read_input).unwrap();
    let workspace = Workspace { platform, yes: true, timeout: Duration::from_millis(100), io_limit: 24000 };
    (dir, workspace)
}

#[tokio::test]
async fn images_check_signatures_size_and_path() {
    let (_dir, ws) = workspace();
    for (bytes, mime) in [
        (b"\x89PNG\r\n\x1a\n".as_slice(), "image/png"),
        (&[0xff, 0xd8, 0xff], "image/jpeg"),
        (b"GIF89a", "image/gif"),
        (b"GIF87a", "image/gif"),
        (b"RIFF1234WEBP", "image/webp"),
    ] {
        assert_eq!(image_mime(bytes).unwrap(), mime);
    }
    assert!(image_mime(b"<svg/>").is_err());
    assert!(ws.image("../escape.png").await.is_err());
    let file = fs::File::create(ws.platform.root().join("large.png")).unwrap();
    file.set_len(8 * 1024 * 1024 + 1).unwrap();
    assert!(ws.image("large.png").await.unwrap_err().to_string().contains("8 MiB"));
}

#[test]
fn protocol_endpoints_and_truncated_tool_responses() {
    assert_eq!(
        endpoint("https://api.deepseek.com/anthropic/", Protocol::Anthropic),
        "https://api.deepseek.com/anthropic/v1/messages"
    );
    assert_eq!(endpoint("http://localhost/v1", Protocol::Anthropic), "http://localhost/v1/messages");
    assert_eq!(endpoint("http://localhost/v1/", Protocol::Openai), "http://localhost/v1/chat/completions");
    let response = json!({"type":"message","role":"assistant","stop_reason":"max_tokens",
        "content":[{"type":"tool_use","id":"x","name":"shell","input":{"command":"echo test"}}]});
    assert!(normalize_response(Protocol::Anthropic, response).is_err());
    let response = json!({"type":"message","role":"assistant","stop_reason":"max_tokens",
        "content":[{"type":"text","text":"unfinished"}]});
    assert_eq!(normalize_response(Protocol::Anthropic, response).unwrap()["choices"][0]["finish_reason"], "max_tokens");
}

#[test]
fn api_config_validation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("api.json");
    fs::write(&path, r#"{"protocol":"anthropic","base_url":"http://localhost","model":"test","api_key_env":"CUSTOM_KEY","max_tokens":1024,"extra_body":{"temperature":0.2}}"#).unwrap();
    let config = load_config(Some(&path)).unwrap();
    assert_eq!(config.protocol.as_deref(), Some("anthropic"));
    assert_eq!(config.max_tokens, Some(1024));
    assert_eq!(config.api_key_env.as_deref(), Some("CUSTOM_KEY"));
    assert_eq!(config.extra_body["temperature"], 0.2);
    fs::write(&path, r#"{"api_key":"direct-key","api_key_env":"FALLBACK_KEY"}"#).unwrap();
    let config = load_config(Some(&path)).unwrap();
    assert_eq!(config.api_key.as_deref(), Some("direct-key"));
    assert_eq!(config.api_key_env.as_deref(), Some("FALLBACK_KEY"));
    for invalid in [
        r#"{"protocol":123}"#,
        r#"{"max_tokens":"wrong"}"#,
        r#"{"network_retries":9}"#,
        r#"{"network_retries":-1}"#,
        r#"{"proxy":true}"#,
    ] {
        fs::write(&path, invalid).unwrap();
        assert!(load_config(Some(&path)).is_err(), "accepted {invalid}");
    }
    assert!(load_config(None).unwrap().model.is_none());
}

#[test]
fn responses_converts_history_and_does_not_leak_protocol_metadata() {
    let history = vec![
        json!({"role":"system","content":"test"}),
        json!({"role":"user","content":"hello"}),
        json!({"role":"assistant","content":null,"tool_calls":[{"id":"c1","type":"function",
            "function":{"name":"read","arguments":"{\"path\":\"a\"}"}}]}),
        json!({"role":"tool","tool_call_id":"c1","content":"ERROR: missing"}),
    ];
    let exec = json!(["../target/debug/i-plugins", "responses"]);
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("plugins");
    let data = rpc(
        &exec,
        &json!({"event":"before_request","data":{
        "url":"http://localhost/v1/chat/completions", "history":history,"compact":false,
        "body":request_body(Protocol::Openai,"model",&history,123).unwrap()}}),
        &dir,
        24000,
    )
    .unwrap();
    let request = &data["body"];
    assert_eq!(request["input"][2]["type"], "function_call");
    assert_eq!(request["input"][2]["call_id"], "c1");
    assert_eq!(request["input"][3]["type"], "function_call_output");
    assert_eq!(request["max_output_tokens"], 123);
    assert_eq!(data["url"], "http://localhost/v1/responses");
    let response = json!({"status":"completed","output":[{"type":"message","role":"assistant",
        "content":[{"type":"refusal","refusal":"Cannot do that"}]}]});
    let data = rpc(&exec, &json!({"event":"after_response","data":{"response":response}}), &dir, 24000).unwrap();
    let message = data["response"]["choices"][0]["message"].clone();
    assert_eq!(message["content"], "Cannot do that");
    let request = request_body(Protocol::Openai, "model", &[message], 123).unwrap();
    assert!(request["messages"][0].get("_responses_output").is_none());
}

#[test]
fn plugin_manifest_validation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plugins.json");
    let valid = json!({"name":"test_plugin","description":"test","parameters":{"type":"object"},"exec":["true"]});
    fs::write(&path, json!({"tools":[valid]}).to_string()).unwrap();
    assert_eq!(load_plugins(Some(&path)).unwrap().tools.len(), 1);
    assert!(load_plugins(None).unwrap().tools.is_empty());
    for bad in [
        json!({"tools":[valid, valid]}),
        json!({"tools":[{"name":"read"}]}),
        json!({}),
        json!({"tools":[{"name":"bad name"}]}),
        json!({"tools":[{"name":"x","description":"x","exec":["true"],"parameters":{"type":"string"}}]}),
    ] {
        fs::write(&path, bad.to_string()).unwrap();
        assert!(load_plugins(Some(&path)).is_err());
    }
}

#[test]
fn history_validator_rejects_orphans_duplicates_and_unfinished_batches() {
    let system = json!({"role":"system","content":"system"});
    let call = json!({"role":"assistant","content":null,"tool_calls":[{"id":"a","type":"function","function":{"name":"read","arguments":"{}"}}]});
    let result = json!({"role":"tool","tool_call_id":"a","content":"done"});
    assert!(checked_history(json!([system, call, result])).is_ok());
    for bad in [
        json!([]),
        json!([system, result]),
        json!([system, call]),
        json!([system,call,{"role":"user","content":"interrupt"},result]),
        json!([system,{"role":"assistant","tool_calls":[{"id":"a"},{"id":"a"}]}]),
        json!([system,{"role":"user","tool_calls":[]}]),
        json!([system,{"role":"assistant","tool_calls":"bad"}]),
    ] {
        assert!(checked_history(bad).is_err());
    }
}

#[test]
fn transcript_redaction_removes_images_and_private_adapter_state() {
    let mut value = json!([{"role":"user","content":[{"type":"image_url","image_url":{"url":"data:image/png;base64,private"}}]},
        {"role":"assistant","content":"visible result","_responses_output":[{"encrypted_content":"private"}]}]);
    value = rpc(
        &json!(["../target/debug/i-plugins", "compact"]),
        &json!({"event":"before_compact","data":{"prompt":"summary","messages":value,"policy":{"images":"omit"}}}),
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("plugins"),
        24000,
    )
    .unwrap();
    assert!(!value.to_string().contains("private"));
    assert!(value.to_string().contains("visible result"));
    assert!(value.to_string().contains("handled separately"));
}

#[test]
fn rpc_rejects_bad_json_failures_and_excessive_output() {
    let dir = tempfile::tempdir().unwrap();
    for code in [
        "print('not json')",
        "raise RuntimeError('fixture failure')",
        "import sys,time; sys.stderr.write('x'*30000); sys.stderr.flush(); time.sleep(30)",
    ] {
        let start = Instant::now();
        assert!(rpc(&json!(["python3", "-c", code]), &json!({}), dir.path(), 24000).is_err());
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}

#[test]
fn worker_pool_reuses_processes_and_separates_working_directories() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("sub")).unwrap();
    let code = "import json,os,sys\nn=0\nfor line in sys.stdin:\n e=json.loads(line); n+=1; print(json.dumps({'id':e['id'],'data':{'pid':os.getpid(),'n':n,'cwd':os.getcwd()}}),flush=True)";
    let entry = json!({"exec":["python3","-c",code]});
    let mut plugins = load_plugins(None).unwrap();
    plugins.dir = dir.path().into();
    let first = plugins.call(&entry, "tool", json!({})).unwrap();
    let second = plugins.call(&entry, "before_turn", json!({})).unwrap();
    assert_eq!(first["pid"], second["pid"]);
    assert_eq!(second["n"], 2);
    let mut other = entry.clone();
    other["cwd"] = json!("sub");
    let third = plugins.call(&other, "tool", json!({})).unwrap();
    assert_ne!(third["pid"], first["pid"]);
    other["cwd"] = json!(".");
    assert_eq!(plugins.call(&other, "tool", json!({})).unwrap()["n"], 3);
    drop(plugins);
    #[cfg(unix)]
    for pid in [first["pid"].to_string(), third["pid"].to_string()] {
        assert!(!Command::new("kill").args(["-0", &pid]).stderr(Stdio::null()).status().unwrap().success());
    }
}
#[test]
fn failed_worker_operations_are_not_replayed_and_next_call_restarts() {
    let dir = tempfile::tempdir().unwrap();
    let code = "import json,os,sys\nfrom pathlib import Path\nfor line in sys.stdin:\n e=json.loads(line)\n with Path('effects').open('a') as f: f.write(str(os.getpid())+'\\n')\n print(json.dumps({'id':e['id'],'error':'side effect already happened'}),flush=True)";
    let mut plugins = load_plugins(None).unwrap();
    plugins.dir = dir.path().into();
    let entry = json!({"exec":["python3","-c",code]});
    assert!(plugins.call(&entry, "tool", json!({})).is_err());
    assert_eq!(fs::read_to_string(dir.path().join("effects")).unwrap().lines().count(), 1);
    assert!(plugins.workers.borrow().is_empty());
    assert!(plugins.call(&entry, "tool", json!({})).is_err());
    let lines = fs::read_to_string(dir.path().join("effects")).unwrap();
    let lines: Vec<_> = lines.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_ne!(lines[0], lines[1]);
}
#[test]
fn worker_timeout_covers_blocked_stdin_and_kills_the_process() {
    let dir = tempfile::tempdir().unwrap();
    let code = "import time\nfrom pathlib import Path\ntime.sleep(0.7)\nPath('late-effect').write_text('bad')";
    let mut worker = Worker::start(&json!(["python3", "-c", code]), dir.path(), 1000).unwrap();
    let start = Instant::now();
    assert!(worker.call(json!({"payload":"x".repeat(1_000_000)}), Duration::from_millis(100)).is_err());
    drop(worker);
    assert!(start.elapsed() < Duration::from_secs(2));
    thread::sleep(Duration::from_millis(800));
    assert!(!dir.path().join("late-effect").exists());
}
#[test]
fn worker_rejects_mismatched_ids_eof_and_oversized_frames() {
    let dir = tempfile::tempdir().unwrap();
    for code in [
        "import json,sys\ne=json.loads(sys.stdin.readline()); print(json.dumps({'id':e['id']+1,'data':{}}),flush=True)",
        "import json,sys\ne=json.loads(sys.stdin.readline()); print(json.dumps({'id':e['id'],'data':[]} ),flush=True)",
        "import sys\nsys.stdin.readline(); sys.stdout.write('x'*(48*1024*1024+1)); sys.stdout.flush()",
        "import sys\nsys.stdin.readline()",
    ] {
        let mut worker = Worker::start(&json!(["python3", "-c", code]), dir.path(), 24000).unwrap();
        assert!(worker.call(json!({}), Duration::from_secs(3)).is_err());
    }
}
#[cfg(unix)]
#[tokio::test]
async fn native_file_reads_reject_pipes_without_blocking() {
    let dir = tempfile::tempdir().unwrap();
    let pipe = dir.path().join("pipe.png");
    assert!(Command::new("mkfifo").arg(&pipe).status().unwrap().success());
    let root = dir.path().to_path_buf();
    let (send, receive) = mpsc::channel();
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let platform = NativePlatform::new(root, readline::read_input).unwrap();
            let workspace = Workspace { platform, yes: false, io_limit: 24000, timeout: Duration::from_secs(1) };
            send.send(workspace.read_text("pipe.png").await.is_err() && workspace.image("pipe.png").await.is_err())
                .unwrap();
        });
    });
    assert!(receive.recv_timeout(Duration::from_secs(2)).unwrap());
}
#[test]
fn clipping_handles_unicode() {
    assert_eq!(clip("你好abc", 4), "你\n[truncated]");
    assert_eq!(clip("abc", 3), "abc");
}

#[test]
fn replacement_must_be_unique() {
    assert_eq!(replace_once("hello world", "world", "Rust", None).unwrap(), "hello Rust");
    assert!(replace_once("aaa", "a", "b", None).is_err());
    assert!(replace_once("aaa", "aa", "b", None).is_err());
    assert!(replace_once("abc", "z", "b", None).is_err());
    assert!(replace_once("abc", "", "b", None).is_err());
}

#[test]
fn replacement_with_line_number() {
    let text = "line1\napi_key_env: Option<String>,\nline3\napi_key_env: Option<String>,\nline5";
    // Replace on line 2
    let result = replace_once(
        text,
        "api_key_env: Option<String>,",
        "api_key: Option<String>,\n    api_key_env: Option<String>,",
        Some(2),
    )
    .unwrap();
    assert_eq!(
        result,
        "line1\napi_key: Option<String>,\n    api_key_env: Option<String>,\nline3\napi_key_env: Option<String>,\nline5"
    );
    // Replace on line 4
    let result = replace_once(
        text,
        "api_key_env: Option<String>,",
        "api_key: Option<String>,\n    api_key_env: Option<String>,",
        Some(4),
    )
    .unwrap();
    assert_eq!(
        result,
        "line1\napi_key_env: Option<String>,\nline3\napi_key: Option<String>,\n    api_key_env: Option<String>,\nline5"
    );
    // Error: old not found on specified line
    assert!(replace_once(text, "notfound", "x", Some(2)).is_err());
    // Error: line out of range
    assert!(replace_once(text, "line1", "x", Some(100)).is_err());
    // Error: old appears multiple times on the same line
    assert!(replace_once("aaa", "a", "b", Some(1)).is_err());
}

#[tokio::test]
async fn file_tools_round_trip() {
    let (_dir, ws) = workspace();
    let plugins = load_plugins(None).unwrap();
    ws.tool("write", &json!({"path":"sub/a.txt","content":"one\ntwo\nthree\n"}), &plugins, 8).await.unwrap();
    let output = ws
        .tool("read", &json!({"files":[{"path":"sub/a.txt","start_line":2,"end_line":2}]}), &plugins, 8)
        .await
        .unwrap();
    let read: Value = serde_json::from_str(output["output"].as_str().unwrap()).unwrap();
    assert_eq!(read["results"][0]["content"], "2: two\n");
    ws.tool("edit", &json!({"path":"sub/a.txt","edits":[{"old":"two","new":"二"}]}), &plugins, 8).await.unwrap();
    assert_eq!(fs::read_to_string(ws.platform.root().join("sub/a.txt")).unwrap(), "one\n二\nthree\n");
    assert!(ws.tool("read", &json!({"files":[{"path":"sub/a.txt","start_line":0}]}), &plugins, 8).await.is_err());
    assert!(ws.tool("unknown", &json!({}), &plugins, 8).await.is_err());
}

#[tokio::test]
async fn native_write_and_edit_keep_original_output_size_policy() {
    let (_dir, ws) = workspace();
    let plugins = load_plugins(None).unwrap();
    let content = "x".repeat(2_000_001);
    ws.tool("write", &json!({"path":"large.txt","content":content}), &plugins, 8).await.unwrap();
    assert_eq!(fs::metadata(ws.platform.root().join("large.txt")).unwrap().len(), 2_000_001);
    fs::write(ws.platform.root().join("small.txt"), "old").unwrap();
    ws.tool("edit", &json!({"path":"small.txt","edits":[{"old":"old","new":content}]}), &plugins, 8).await.unwrap();
    assert_eq!(fs::metadata(ws.platform.root().join("small.txt")).unwrap().len(), 2_000_001);
}

#[tokio::test]
async fn rejects_escaping_paths() {
    let (_dir, ws) = workspace();
    for path in ["../secret", "/etc/passwd", "a/../../b", ""] {
        assert!(ws.path(path).await.is_err(), "allowed {path}");
    }
    assert!(ws.path("new/nested/file").await.is_ok());
}

#[cfg(unix)]
#[tokio::test]
async fn rejects_symlink_escape_and_dangling_link() {
    use std::os::unix::fs::symlink;
    let (_dir, ws) = workspace();
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), ws.platform.root().join("escape")).unwrap();
    assert!(ws.path("escape/new/file").await.is_err());
    symlink(outside.path().join("missing"), ws.platform.root().join("dangling")).unwrap();
    assert!(ws.path("dangling").await.is_err());
}

#[tokio::test]
async fn session_round_trip_preserves_request_bytes_and_argument_text() {
    let (_dir, ws) = workspace();
    let rel_path = Path::new("round-trip.json");
    let arguments = r#"{ "z": 1, "a": { "b": 2, "a": "中文" } }"#;
    let history = vec![
        json!({"role":"system","content":"Stable system prefix."}),
        json!({"role":"user","content":"保留格式\n  indented text"}),
        json!({"role":"assistant","content":null,"tool_calls":[{
            "id":"call_stable","type":"function","function":{"name":"write","arguments":arguments}}]}),
        json!({"role":"tool","tool_call_id":"call_stable","content":"{\"z\":1, \"a\":2}"}),
    ];
    let mut restored = history.clone();
    for _ in 0..3 {
        save_session(Some(rel_path), ws.platform.root(), &restored, &ws).await.unwrap();
        restored = load_session(Some(rel_path), ws.platform.root(), &ws).await.unwrap();
        assert_eq!(restored, history);
        assert_eq!(restored[2]["tool_calls"][0]["function"]["arguments"], arguments);
        for protocol in [Protocol::Openai, Protocol::Anthropic] {
            let before = request_body(protocol, "test-model", &history, 1024).unwrap();
            let after = request_body(protocol, "test-model", &restored, 1024).unwrap();
            assert_eq!(serde_json::to_vec(&before).unwrap(), serde_json::to_vec(&after).unwrap());
        }
    }
}
#[tokio::test]
async fn sessions_round_trip_and_reject_different_workspace() {
    let (_dir, ws) = workspace();
    let rel_path = Path::new("state/session.json");
    let mut messages = load_session(Some(rel_path), ws.platform.root(), &ws).await.unwrap();
    messages.push(json!({"role":"user","content":"hello"}));
    save_session(Some(rel_path), ws.platform.root(), &messages, &ws).await.unwrap();
    assert_eq!(load_session(Some(rel_path), ws.platform.root(), &ws).await.unwrap(), messages);
    assert!(load_session(Some(rel_path), Path::new("/different"), &ws).await.is_err());
    messages.push(json!({"role":"assistant","content":"done"}));
    save_session(Some(rel_path), ws.platform.root(), &messages, &ws).await.unwrap();
    assert_eq!(load_session(Some(rel_path), ws.platform.root(), &ws).await.unwrap(), messages);
}

#[tokio::test]
async fn shell_runs_on_native_platform() {
    let (_dir, mut ws) = workspace();
    ws.timeout = Duration::from_secs(5);
    let output = ws.shell("echo portable").await.unwrap();
    assert!(output.contains("portable") && output.contains("timed_out: false"), "{output}");
}

#[cfg(windows)]
#[tokio::test]
async fn windows_shell_times_out_process_tree() {
    let (_dir, ws) = workspace();
    let start = Instant::now();
    let output = ws.shell("ping -n 30 127.0.0.1 > nul").await.unwrap();
    assert!(output.contains("timed_out: true"));
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[cfg(unix)]
#[tokio::test]
async fn shell_captures_output_and_exit_status() {
    let (_dir, mut ws) = workspace();
    ws.timeout = Duration::from_secs(5);
    let output = ws.shell("printf hello; printf error >&2; exit 7").await.unwrap();
    assert!(output.contains("exit status: 7"), "{output}");
    assert!(output.contains("stdout:\nhello"));
    assert!(output.contains("stderr:\nerror"));
}

#[cfg(unix)]
#[tokio::test]
async fn shell_times_out_pipelines() {
    let (_dir, ws) = workspace();
    let start = Instant::now();
    let output = ws.shell("sleep 10 | sleep 10").await.unwrap();
    assert!(output.contains("timed_out: true"), "{output}");
    assert!(start.elapsed() < Duration::from_secs(3));
}

#[cfg(unix)]
#[tokio::test]
async fn shell_drains_large_output_without_deadlock() {
    let (_dir, mut ws) = workspace();
    ws.timeout = Duration::from_secs(5);
    let output = ws.shell("head -c 100000 /dev/zero").await.unwrap();
    assert!(output.contains("timed_out: false"));
    assert!(output.contains("[truncated]"));
    assert!(output.len() < ws.io_limit + 500);
}
