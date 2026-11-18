use super::*;
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    sync::atomic::{AtomicBool, Ordering},
    thread,
};

fn budget_data(used: u64) -> Value {
    let mut history = json!([{"role":"system","content":"system"},{"role":"user","content":"task"}]);
    let anchor = json!({"model":"fixture","history_hash":budget::fingerprint(&history)});
    history.as_array_mut().unwrap().push(
        json!({"role":"assistant","content":"answer","_usage":{"tokens":{"prompt_tokens":used,"completion_tokens":0},"anchor":anchor}}),
    );
    json!({"history":history,"model":"fixture","max_tokens":100,"policy":{"context_window":1000},"tools":[],"plugin_tools":[],"pending_tools":1})
}
#[test]
fn budget_eighty_percent_and_output_reservation() {
    let result = budget::budget(budget_data(790)).unwrap();
    assert_eq!(result["compact"], false);
    assert_eq!(result["input_trigger"], 800);
    assert_eq!(result["source"], "usage+estimate");
    assert_eq!(budget::budget(budget_data(800)).unwrap()["compact"], true);
    let mut data = budget_data(550);
    data["max_tokens"] = json!(450);
    let result = budget::budget(data).unwrap();
    assert_eq!(result["input_trigger"], 530);
    assert_eq!(result["fits"], false);
}
#[test]
fn budget_tail_and_anchor_validation() {
    let mut data = budget_data(700);
    data["history"].as_array_mut().unwrap().push(json!({"role":"user","content":"x".repeat(150)}));
    assert_eq!(budget::budget(data).unwrap()["compact"], true);
    let mut data = budget_data(700);
    data["history"][1]["content"] = json!("different");
    assert_eq!(budget::budget(data).unwrap()["source"], "estimate");
}
#[test]
fn cached_tokens_counted_once() {
    assert_eq!(
        budget::usage_total(
            &json!({"prompt_tokens":1000,"completion_tokens":50,"prompt_tokens_details":{"cached_tokens":900}})
        ),
        Some(1050)
    );
    assert_eq!(
        budget::usage_total(
            &json!({"input_tokens":1000,"output_tokens":50,"input_tokens_details":{"cached_tokens":900}})
        ),
        Some(1050)
    );
    assert_eq!(
        budget::usage_total(
            &json!({"input_tokens":10,"output_tokens":20,"cache_read_input_tokens":900,"cache_creation_input_tokens":100})
        ),
        Some(1030)
    );
    assert_eq!(budget::usage_total(&json!({"input_tokens":-1})), None);
}
#[test]
fn dynamic_tool_budget_and_validation() {
    let mut data = budget_data(790);
    data["pending_tools"] = json!(10);
    assert_eq!(budget::budget(data).unwrap()["tool_output_bytes"], 1);
    let mut data = budget_data(0);
    assert_eq!(budget::budget(data.clone()).unwrap()["tool_output_tokens"], 20);
    data["policy"]["context_window"] = json!(10000);
    assert_eq!(budget::budget(data).unwrap()["tool_output_tokens"], 200);
    for policy in [
        json!({}),
        json!({"context_window":100}),
        json!({"context_window":1000,"compact_ratio":1}),
        json!({"context_window":1000,"safety_ratio":-1}),
    ] {
        let mut data = budget_data(0);
        data["policy"] = policy;
        assert!(budget::budget(data).is_err());
    }
    let mut data = budget_data(0);
    data["policy"] = json!({"enabled":false});
    assert_eq!(budget::budget(data).unwrap()["source"], "disabled");
    let mut data = budget_data(0);
    data["policy"] = json!({"models":{"fixture":{"context_window":2000}}});
    assert_eq!(budget::budget(data).unwrap()["context_window"], 2000);
}
#[test]
fn image_policies_and_explicit_estimates() {
    let image = json!({"type":"image_url","image_url":{"url":"data:image/png;base64,ORIGINAL"}});
    for mode in ["keep", "omit", "describe"] {
        let result = super::compact::prepare(json!({"policy":{"images":mode},"prompt":"summary","messages":[
            {"role":"user","content":[image]}, {"role":"assistant","content":"visible","_responses_output":["secret state"]}]})).unwrap();
        assert!(!result["messages"].to_string().contains("ORIGINAL"));
        assert!(!result.to_string().contains("secret state"));
        assert_eq!(result["retained_images"], if mode == "keep" { json!([image]) } else { json!([]) });
        assert_eq!(result["summary_images"], if mode == "describe" { json!([image]) } else { json!([]) });
    }
    assert!(budget::Counter::new(&json!({})).unwrap().count(&image).is_err());
    assert_eq!(budget::Counter::new(&json!({"image_tokens":777})).unwrap().count(&image).unwrap(), 777);
}
#[test]
fn conservative_counter_needs_no_vocabulary() {
    let counter = budget::Counter::new(&json!({})).unwrap();
    assert_eq!(counter.count(&json!("hello world")).unwrap(), 11);
    assert_eq!(counter.count(&json!("你好")).unwrap(), 6);
    for name in ["cl100k_base", "o200k_base", "p50k_base", "r50k_base", "missing"] {
        assert!(budget::Counter::new(&json!({"tokenizer":name})).is_err());
    }
}
#[test]
fn usage_is_primary_but_unknown_usage_and_new_content_are_not_free() {
    let mut data = budget_data(700);
    data["history"][1]["content"] = json!("x".repeat(50000));
    data["history"][2]["_usage"]["anchor"]["history_hash"] =
        budget::fingerprint(&json!(data["history"].as_array().unwrap()[..2])).into();
    let result = budget::budget(data.clone()).unwrap();
    assert_eq!(result["source"], "usage+estimate");
    assert_eq!(result["estimated_input_tokens"], 700);
    assert_eq!(result["fits"], true);
    assert!(result["content_estimate_tokens"].as_u64().unwrap() > 50000);
    let mut grown = data.clone();
    grown["history"].as_array_mut().unwrap().push(json!({"role":"user","content":"新".repeat(100)}));
    assert_eq!(budget::budget(grown).unwrap()["fits"], false);
    let mut different_model = data.clone();
    different_model["model"] = json!("different");
    assert_eq!(budget::budget(different_model).unwrap()["source"], "estimate");
    for usage in [
        json!(null),
        json!({}),
        json!({"prompt_tokens":700}),
        json!({"input_tokens":700}),
        json!({"prompt_tokens":700,"completion_tokens":-1}),
        json!({"prompt_tokens":u64::MAX,"completion_tokens":1}),
    ] {
        data["history"][2]["_usage"]["tokens"] = usage;
        let fallback = budget::budget(data.clone()).unwrap();
        assert_eq!(fallback["source"], "estimate");
        assert_eq!(fallback["fits"], false);
    }
}
fn response_request(compact: bool) -> Value {
    json!({"url":"https://example.com/v1/chat/completions","compact":compact,
        "history":[{"role":"user","content":[{"type":"text","text":"inspect"},{"type":"image_url","image_url":{"url":"data:image/png;base64,eA==","detail":"low"}}]}],
        "body":{"model":"test","max_completion_tokens":123,"reasoning":{"effort":"medium"},"include":["reasoning.encrypted_content"],
        "tools":[{"type":"function","function":{"name":"read","parameters":{"type":"object"}}}]}})
}
#[test]
fn responses_maps_images_tokens_tools_and_compaction() {
    let data = responses::handle("before_request", response_request(false)).unwrap();
    assert_eq!(data["url"], "https://example.com/v1/responses");
    assert_eq!(data["body"]["input"][0]["content"][1]["image_url"], "data:image/png;base64,eA==");
    assert_eq!(data["body"]["input"][0]["content"][1]["detail"], "low");
    assert_eq!(data["body"]["max_output_tokens"], 123);
    assert_eq!(data["body"]["reasoning"], json!({"effort":"medium"}));
    assert_eq!(data["body"]["tools"][0]["strict"], false);
    assert_eq!(data["body"]["store"], false);
    let data = responses::handle("before_request", response_request(true)).unwrap();
    assert!(data["body"].get("tools").is_none());
    assert!(data["body"].get("max_completion_tokens").is_none());
}
#[test]
fn responses_replays_reasoning_and_rejects_incomplete() {
    let output = json!([{"type":"reasoning","encrypted_content":"opaque","content":[{"type":"reasoning_text","text":"plan"}]},
        {"type":"function_call","id":"item1","call_id":"call1","name":"read","arguments":"{}"}]);
    let response = responses::normalize(&json!({"status":"completed","output":output})).unwrap();
    let items = responses::input(
        &json!([response["choices"][0]["message"],{"role":"tool","tool_call_id":"call1","content":"result"}]),
    )
    .unwrap();
    assert_eq!(items[0], output[0]);
    assert_eq!(items[1], output[1]);
    assert_eq!(items[2]["call_id"], "call1");
    for status in ["failed", "incomplete", "in_progress"] {
        assert!(responses::normalize(&json!({"status":status,"output":[]})).is_err());
    }
    assert!(responses::normalize(&json!({"status":"completed","output":[{"type":"web_search_call"}]})).is_err());
    assert!(
        responses::normalize(&json!({"status":"completed","output":[{"type":"function_call","arguments":{}}]}))
            .is_err()
    );
}
fn memory_tool(root: &Path, name: &str, arguments: Value) -> Value {
    memory::handle("tool", json!({"name":name,"arguments":arguments}), root).unwrap()
}
#[test]
fn memory_roundtrip_permissions_and_no_implicit_files() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        memory::handle("system_prompt", json!({"prompt":"base"}), dir.path()).unwrap(),
        json!({"prompt":"base"})
    );
    assert!(!dir.path().join("memory.json").exists());
    memory_tool(dir.path(), "memory_set", json!({"key":"lang","value":"中文 Rust"}));
    assert_eq!(memory_tool(dir.path(), "memory_get", json!({"key":"lang"}))["output"], json!("中文 Rust").to_string());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(dir.path().join("memory.json")).unwrap().permissions().mode() & 0o777, 0o600);
    }
    let data =
        memory::handle("before_turn", json!({"messages":[{"role":"system","content":"base"}]}), dir.path()).unwrap();
    assert_eq!(data["messages"][0]["content"], "base");
    assert!(data["messages"][1]["content"].as_str().unwrap().contains("Untrusted"));
    assert!(data["messages"][1]["content"].as_str().unwrap().contains("中文 Rust"));
    assert_eq!(memory::handle("before_turn", data.clone(), dir.path()).unwrap(), data);
    memory_tool(dir.path(), "memory_delete", json!({"key":"lang"}));
    assert_eq!(memory_tool(dir.path(), "memory_get", json!({}))["output"], "{}");
}
#[test]
fn memory_validation_and_concurrent_updates() {
    let dir = tempfile::tempdir().unwrap();
    memory_tool(dir.path(), "memory_set", json!({"key":"keep","value":"safe"}));
    for args in [
        json!({"key":"../escape","value":"x"}),
        json!({"key":"ok","value":"x".repeat(2001)}),
        json!({"key":"ok","value":42}),
    ] {
        assert!(memory::handle("tool", json!({"name":"memory_set","arguments":args}), dir.path()).is_err());
    }
    assert_eq!(memory_tool(dir.path(), "memory_get", json!({}))["output"], "{\"keep\":\"safe\"}");
    thread::scope(|scope| {
        for number in 0..10 {
            let root = dir.path();
            scope.spawn(move || {
                memory_tool(root, "memory_set", json!({"key":format!("k{number}"),"value":number.to_string()}));
            });
        }
    });
    let result: Value =
        serde_json::from_str(memory_tool(dir.path(), "memory_get", json!({}))["output"].as_str().unwrap()).unwrap();
    assert_eq!(result.as_object().unwrap().len(), 11);
}
#[cfg(unix)]
#[test]
fn memory_rejects_symlink_escape() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("memory.json")).unwrap();
    assert!(
        memory::handle("tool", json!({"name":"memory_set","arguments":{"key":"x","value":"x"}}), root.path()).is_err()
    );
}
#[test]
fn prompts_use_two_files_and_validate_contents() {
    let dir = tempfile::tempdir().unwrap();
    let system = dir.path().join("system.txt");
    let compact = dir.path().join("compact.txt");
    fs::write(&system, "System for {{workspace}}").unwrap();
    fs::write(&compact, "Compact transcript").unwrap();
    let args = vec![
        "--system".into(),
        system.to_string_lossy().into_owned(),
        "--compact".into(),
        compact.to_string_lossy().into_owned(),
    ];
    for (event, expected) in [("system_prompt", "System for /repo"), ("before_compact", "Compact transcript")] {
        assert_eq!(prompts::handle(event, json!({"prompt":"old"}), "/repo", &args).unwrap()["prompt"], expected);
    }
    fs::write(compact, "").unwrap();
    assert!(prompts::handle("before_compact", json!({}), "/repo", &args).is_err());
}
#[test]
fn steering_acknowledges_saved_ids_only() {
    let dir = tempfile::tempdir().unwrap();
    let inbox = dir.path().join("inbox.json");
    let id = steering::enqueue(&inbox, "change direction").unwrap();
    let data =
        steering::handle("before_turn", json!({"messages":[{"role":"system","content":"system"}]}), &inbox).unwrap();
    assert_eq!(data["messages"][1]["_steering_id"], id);
    assert_eq!(steering::pending(&inbox).unwrap().len(), 1);
    assert_eq!(steering::handle("before_turn", data.clone(), &inbox).unwrap()["messages"].as_array().unwrap().len(), 2);
    steering::handle("after_save", json!({"messages":[]}), &inbox).unwrap();
    assert_eq!(steering::pending(&inbox).unwrap().len(), 1);
    steering::handle("after_save", data, &inbox).unwrap();
    assert!(steering::pending(&inbox).unwrap().is_empty());
}
#[test]
fn console_only_accepts_authenticated_stderr_approval_marker() {
    let approval = AtomicBool::new(false);
    let mut output = Vec::new();
    console::relay("Allow? [y/N]".as_bytes(), &mut output, &approval, Some("secret-token")).unwrap();
    assert!(!approval.load(Ordering::Acquire));
    console::relay("\x1esecret-token\x1eAllow?".as_bytes(), &mut output, &approval, Some("secret-token")).unwrap();
    assert!(approval.load(Ordering::Acquire));
    assert!(!String::from_utf8(output).unwrap().contains("secret-token"));
    approval.store(false, Ordering::Release);
    console::relay("\x1esecret-token\x1e".as_bytes(), Vec::new(), &approval, None).unwrap();
    assert!(!approval.load(Ordering::Acquire));
}
fn tool_output(value: Value) -> Value {
    serde_json::from_str(value["output"].as_str().unwrap()).unwrap()
}
#[test]
fn code_search_literals_regex_glob_and_limits() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("main.rs"), "fn main() {}\nNeedle 123\nneedle 456\n").unwrap();
    fs::write(dir.path().join(".env"), "needle secret").unwrap();
    fs::create_dir(dir.path().join("target")).unwrap();
    fs::write(dir.path().join("target/ignored.rs"), "needle").unwrap();
    let search = |args: Value| {
        tool_output(search::handle("tool", json!({"name":"search","arguments":args}), dir.path()).unwrap())
    };
    let result = search(json!({"query":"needle","glob":"*.rs"}));
    assert_eq!(result["matches"].as_array().unwrap().len(), 2);
    assert_eq!(result["matches"][0]["line"], 2);
    assert_eq!(result["matches"][1]["path"], "main.rs");
    assert_eq!(search(json!({"query":"needle \\d+","regex":true,"case_sensitive":true}))["matches"][0]["line"], 3);
    assert_eq!(search(json!({"query":"needle","limit":1}))["truncated"], true);
    for arguments in [json!({"query":"x","path":"../escape"}), json!({"query":""}), json!({"query":"x","limit":0})] {
        assert!(search::handle("tool", json!({"name":"search","arguments":arguments}), dir.path()).is_err());
    }
    let files = tool_output(
        search::handle("tool", json!({"name":"find_files","arguments":{"glob":"*.rs"}}), dir.path()).unwrap(),
    );
    assert_eq!(files["matches"], json!([{"path":"main.rs"}]));
}
fn put_skill(root: &Path, description: &str) {
    let dir = root.join(".agents/skills/testing");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("SKILL.md"),
        format!(
            "---\nname: testing\ndescription: {description}\n---\nPrivate body marker. Scripts require approval.\n"
        ),
    )
    .unwrap();
    fs::write(dir.join("reference.txt"), "reference contents").unwrap();
}
#[test]
fn skills_load_on_demand_and_refresh_without_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    let initial = json!({"messages":[{"role":"system","content":"base"}]});
    assert_eq!(
        skills::handle("before_turn", initial.clone(), dir.path(), &[]).unwrap()["messages"][0]["content"],
        "base"
    );
    put_skill(dir.path(), "initial description");
    let data = skills::handle("before_turn", initial, dir.path(), &[]).unwrap();
    assert_eq!(data["messages"][0]["content"], "base");
    let reference = data["messages"][1].clone();
    assert!(reference["content"].as_str().unwrap().contains("initial description"));
    assert!(!reference["content"].as_str().unwrap().contains("Private body marker"));
    assert_eq!(skills::handle("before_turn", data.clone(), dir.path(), &[]).unwrap(), data);
    put_skill(dir.path(), "updated description");
    let data = skills::handle("before_turn", data, dir.path(), &[]).unwrap();
    assert_eq!(data["messages"][1], reference);
    let delta = data["messages"][2]["content"].as_str().unwrap();
    assert!(!delta.contains("initial description"));
    assert!(delta.contains("updated description") && delta.contains("Kind: delta"));
    let result = tool_output(
        skills::handle("tool", json!({"name":"skill_load","arguments":{"name":"testing"}}), dir.path(), &[]).unwrap(),
    );
    assert!(result["content"].as_str().unwrap().contains("Private body marker"));
    assert!(result["trust"].as_str().unwrap().contains("untrusted"));
    let reference = tool_output(
        skills::handle(
            "tool",
            json!({"name":"skill_read","arguments":{"name":"testing","path":"reference.txt"}}),
            dir.path(),
            &[],
        )
        .unwrap(),
    );
    assert_eq!(reference["content"], "reference contents");
    assert!(
        skills::handle(
            "tool",
            json!({"name":"skill_read","arguments":{"name":"testing","path":"../outside"}}),
            dir.path(),
            &[]
        )
        .is_err()
    );
    fs::remove_dir_all(dir.path().join(".agents/skills/testing")).unwrap();
    let removed = skills::handle("before_turn", data, dir.path(), &[]).unwrap();
    assert_eq!(removed["messages"][0]["content"], "base");
    assert!(removed["messages"][3]["content"].as_str().unwrap().contains("\"remove\":[\"testing\"]"));
}
#[test]
fn skills_explicit_roots_and_duplicate_names() {
    let root = tempfile::tempdir().unwrap();
    put_skill(root.path(), "description");
    let config = root.path().join("skills.json");
    fs::write(&config, json!({"roots":[".agents/skills"]}).to_string()).unwrap();
    let args = vec!["--config".into(), config.to_string_lossy().into_owned()];
    let result = tool_output(
        skills::handle("tool", json!({"name":"skill_list","arguments":{}}), Path::new("."), &args).unwrap(),
    );
    assert_eq!(result[0]["name"], "testing");
    let other = root.path().join(".agents/skills/duplicate");
    fs::create_dir(&other).unwrap();
    fs::copy(root.path().join(".agents/skills/testing/SKILL.md"), other.join("SKILL.md")).unwrap();
    assert!(skills::handle("tool", json!({"name":"skill_list","arguments":{}}), root.path(), &[]).is_err());
}
#[test]
fn web_blocks_non_public_addresses_and_unconfigured_search() {
    for ip in [
        "127.0.0.1",
        "10.0.0.1",
        "169.254.169.254",
        "100.64.0.1",
        "192.0.2.1",
        "198.18.0.1",
        "::1",
        "::ffff:127.0.0.1",
        "fc00::1",
        "2001:db8::1",
        "2001:2::1",
        "3fff::1",
        "64:ff9b::7f00:1",
    ] {
        assert!(!web::public_ip(ip.parse().unwrap()), "{ip}");
    }
    for address in ["8.8.8.8", "192.0.78.24", "2001:4860:4860::8888"] {
        assert!(web::public_ip(address.parse().unwrap()), "{address}");
    }
    for url in ["file:///etc/passwd", "http://user:password@example.com", "http://127.0.0.1:1"] {
        assert!(web::handle("tool", json!({"name":"web_fetch","arguments":{"url":url}}), &[]).is_err());
    }
    assert!(web::handle("tool", json!({"name":"web_search","arguments":{"query":"query"}}), &[]).is_err());
}
fn web_server(content: &'static str, mime: &'static str) -> (String, thread::JoinHandle<String>) {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = server.local_addr().unwrap();
    let thread = thread::spawn(move || {
        let (mut connection, _) = server.accept().unwrap();
        connection.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
        let mut bytes = Vec::new();
        let mut byte = [0];
        while !bytes.ends_with(b"\r\n\r\n") {
            assert_ne!(connection.read(&mut byte).unwrap(), 0);
            bytes.push(byte[0]);
        }
        let length = String::from_utf8_lossy(&bytes)
            .lines()
            .find_map(|line| {
                let (key, value) = line.split_once(':')?;
                key.eq_ignore_ascii_case("content-length").then(|| value.trim().parse::<usize>().unwrap())
            })
            .unwrap_or(0);
        let mut body = vec![0; length];
        connection.read_exact(&mut body).unwrap();
        bytes.extend(body);
        write!(
            connection,
            "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{content}",
            content.len()
        )
        .unwrap();
        String::from_utf8(bytes).unwrap()
    });
    (format!("http://{address}/search"), thread)
}
#[test]
fn web_search_uses_configured_endpoint_credential_and_mapping() {
    let root = tempfile::tempdir().unwrap();
    let (url, server) = web_server(
        r#"{"web":{"results":[{"title":"Title","url":"https://example.com","description":"Snippet"}]}}"#,
        "application/json",
    );
    fs::write(root.path().join("key.txt"), "fixture-secret\n").unwrap();
    let config = root.path().join("web.json");
    fs::write(&config,json!({"endpoint":url,"allow_private_network":true,"results_pointer":"/web/results","snippet_field":"description","auth":{"file":"key.txt","header":"X-Search-Key"}}).to_string()).unwrap();
    let args = vec!["--config".into(), config.to_string_lossy().into_owned()];
    let result = tool_output(
        web::handle("tool", json!({"name":"web_search","arguments":{"query":"rust tests","limit":1}}), &args).unwrap(),
    );
    assert_eq!(result["results"][0]["snippet"], "Snippet");
    let request = server.join().unwrap();
    assert!(request.contains("q=rust+tests") && request.contains("count=1"));
    assert!(request.to_lowercase().contains("x-search-key: fixture-secret"));
    assert!(!result.to_string().contains("fixture-secret"));
}
#[test]
fn web_search_post_uses_json_and_errors_on_wrong_schema() {
    let root = tempfile::tempdir().unwrap();
    for (content, valid) in [(r#"{"results":[]}"#, true), (r#"{"unexpected":[]}"#, false)] {
        let (url, server) = web_server(content, "application/json");
        let config = root.path().join("web.json");
        fs::write(&config, json!({"endpoint":url,"allow_private_network":true,"method":"POST","query_param":"query","limit_param":"max_results","extra":{"search_depth":"basic"}}).to_string()).unwrap();
        let result = web::handle(
            "tool",
            json!({"name":"web_search","arguments":{"query":"test","limit":2}}),
            &["--config".into(), config.to_string_lossy().into_owned()],
        );
        assert_eq!(result.is_ok(), valid);
        let request = server.join().unwrap();
        assert!(request.starts_with("POST /search "));
        let body: Value = serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(body, json!({"query":"test","max_results":2,"search_depth":"basic"}));
    }
}

#[test]
fn web_fetch_revalidates_redirects_and_enforces_response_bound() {
    for (status, headers, content) in [
        ("302 Found", "Location: file:///etc/passwd\r\n", String::new()),
        ("200 OK", "Content-Type: text/plain\r\n", "x".repeat(1_000_001)),
        ("200 OK", "Content-Type: application/octet-stream\r\n", "binary".into()),
    ] {
        let root = tempfile::tempdir().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let worker = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
            let mut head = Vec::new();
            let mut byte = [0];
            while !head.ends_with(b"\r\n\r\n") {
                assert_ne!(socket.read(&mut byte).unwrap(), 0);
                head.push(byte[0]);
            }
            let _ = write!(
                socket,
                "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{content}",
                content.len()
            );
        });
        let config = root.path().join("web.json");
        fs::write(&config, r#"{"allow_private_network":true}"#).unwrap();
        assert!(
            web::handle(
                "tool",
                json!({"name":"web_fetch","arguments":{"url":format!("http://{address}/page")}}),
                &["--config".into(), config.to_string_lossy().into_owned()]
            )
            .is_err()
        );
        worker.join().unwrap();
    }
}

#[cfg(unix)]
#[test]
fn skill_resources_cannot_escape_through_symlinks() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    put_skill(root.path(), "description");
    fs::write(outside.path().join("secret"), "not exposed").unwrap();
    std::os::unix::fs::symlink(outside.path().join("secret"), root.path().join(".agents/skills/testing/link.txt"))
        .unwrap();
    assert!(
        skills::handle(
            "tool",
            json!({"name":"skill_read","arguments":{"name":"testing","path":"link.txt"}}),
            root.path(),
            &[]
        )
        .is_err()
    );
}

#[test]
fn web_fetch_returns_bounded_untrusted_text() {
    let root = tempfile::tempdir().unwrap();
    let (url, server) = web_server("<h1>Page</h1>", "text/html");
    let config = root.path().join("web.json");
    fs::write(&config, r#"{"allow_private_network":true}"#).unwrap();
    let result = tool_output(
        web::handle(
            "tool",
            json!({"name":"web_fetch","arguments":{"url":url}}),
            &["--config".into(), config.to_string_lossy().into_owned()],
        )
        .unwrap(),
    );
    assert_eq!(result["text"], "<h1>Page</h1>");
    assert_eq!(result["truncated"], false);
    server.join().unwrap();
}
