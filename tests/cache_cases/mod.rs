use super::*;

#[test]
fn standard_pipeline_preserves_wire_prefix_across_tools_and_session_restore() {
    check_pipeline(false);
}
#[test]
fn thinking_and_vision_preserve_reasoning_and_image_prefix_across_resume() {
    check_pipeline(true);
}
fn check_pipeline(vision: bool) {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aN1cAAAAASUVORK5CYII=";
    let reasoning = "private-fixture-reasoning 中文\n  preserve spaces";
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let mut command = Command::new(plugin_binary());
    support::isolate(&mut command, root);
    let manifest = command.args(["manifest", "standard"]).output().unwrap();
    assert!(manifest.status.success());
    fs::write(root.join("plugins.json"), manifest.stdout).unwrap();
    let config = root.join("api.json");
    fs::write(
        &config,
        json!({"plugins":"plugins.json","max_tokens":128,"use_max_tokens":true,
        "extra_body":{"thinking":{"type":if vision {"enabled"} else {"disabled"}}},
        "budget":{"context_window":1000000,"compact_ratio":0.9,"safety_ratio":0.02,"image_tokens":1000}})
        .to_string(),
    )
    .unwrap();
    fs::write(root.join("file.txt"), "{ \"z\":1, \"a\":2 }\n中文").unwrap();
    let session = root.join("session.json");
    fs::write(root.join("pixel.png"), STANDARD.decode(png).unwrap()).unwrap();
    let arguments =
        if vision { "{  \"path\" : \"pixel.png\"  }" } else { "{  \"files\" : [{\"path\":\"file.txt\"}]  }" };
    let mut first = tool_reply(if vision { "view_image" } else { "read" }, json!({}));
    first["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"] = json!(arguments);
    first["usage"] = json!({"prompt_tokens":100000,"prompt_cache_hit_tokens":0,"prompt_cache_miss_tokens":100000});
    let mut second = final_reply();
    second["usage"] = json!({"prompt_tokens":101000,"prompt_cache_hit_tokens":100000,"prompt_cache_miss_tokens":1000});
    if vision {
        first["choices"][0]["message"]["reasoning_content"] = json!(reasoning);
        second["choices"][0]["message"]["reasoning_content"] = json!("final-answer-reasoning-fixture");
    }
    let mut third = final_reply();
    third["usage"] = json!({"prompt_tokens":102000,"prompt_cache_hit_tokens":0,"prompt_cache_miss_tokens":102000});
    let (base, server) = server(vec![(200, first), (200, second), (200, third)]);
    for task in ["read file", "continue"] {
        let mut args = vec!["--config", config.to_str().unwrap(), "--session", session.to_str().unwrap()];
        if vision && task == "read file" {
            args.extend(["--image", "pixel.png"]);
        }
        args.push(task);
        let output = run(root, &base, &args);
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        assert!(!String::from_utf8_lossy(&output.stdout).contains(reasoning));
        assert!(!String::from_utf8_lossy(&output.stderr).contains(reasoning));
        // Change disk formatting between processes, without changing the stored values.
        let saved: Value = serde_json::from_slice(&fs::read(&session).unwrap()).unwrap();
        fs::write(&session, serde_json::to_vec(&saved).unwrap()).unwrap();
    }
    let requests = server.join().unwrap();
    for pair in requests.windows(2) {
        let previous = pair[0]["messages"].as_array().unwrap();
        let next = pair[1]["messages"].as_array().unwrap();
        assert!(next.len() > previous.len());
        assert_eq!(serde_json::to_vec(previous).unwrap(), serde_json::to_vec(&next[..previous.len()]).unwrap());
        assert_eq!(pair[0]["tools"], pair[1]["tools"]);
    }
    assert!(
        requests[2]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["tool_calls"][0]["function"]["arguments"] == arguments)
    );
    for request in &requests {
        assert!(request.get("prompt_cache_key").is_none());
        assert!(request.get("cache_control").is_none());
        assert!(!request.to_string().contains("_cache_session"));
        assert!(request.get("request_meta").is_none());
    }
    if vision {
        let url = format!("data:image/png;base64,{png}");
        for request in &requests {
            assert_eq!(request["thinking"]["type"], "enabled");
            assert_eq!(request["messages"][1]["content"][1]["image_url"]["url"], url);
        }
        let restored = requests[2]["messages"].as_array().unwrap();
        assert!(restored.iter().any(|m| m["reasoning_content"] == reasoning));
        assert!(restored.iter().any(|m| m["reasoning_content"] == "final-answer-reasoning-fixture"));
        assert_eq!(
            restored
                .iter()
                .filter(|m| m["content"]
                    .as_array()
                    .is_some_and(|parts| parts.iter().any(|p| p["image_url"]["url"] == url)))
                .count(),
            2
        );
        assert!(restored.iter().filter(|m| m["role"] == "tool").all(|m| m["content"].is_string()));
    }
    let mut command = Command::new(plugin_binary());
    support::isolate(&mut command, root);
    let output = command.args(["cache-report", "-C", root.to_str().unwrap()]).output().unwrap();
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["totals"]["input_tokens"], 303000);
    assert_eq!(report["totals"]["uncached_input_tokens"], 203000);
    assert_eq!(report["phases"]["append"]["prepared_requests"], 2);
    assert_eq!(report["recent"][2]["prefix_diagnostic"], "previous_message_prefix_preserved");
    assert_eq!(report["recent"][2]["cache_read_tokens"], 0); // Stable prefix is not a promised hit.
    assert_eq!(report["recent"][2]["request_hash"].as_str().unwrap().len(), 64);
    assert!(!report.to_string().contains(reasoning));
    assert!(!report.to_string().contains(png));
}
