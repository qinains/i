#[cfg(unix)]
use super::skills;
use super::{brand, budget, cache, context, memory, paths::AppPaths, state};
#[test]
fn query_encoding_matches_the_previous_serializer() {
    fn check(value: Value) {
        let payload = json!({"a +&中文":"x\n\u{0000}😀", "value":value});
        let payload = payload.as_object().unwrap();
        let old = serde_urlencoded::to_string(payload);
        let new = super::web::query_string(payload);
        match (old, new) {
            (Ok(old), Ok(new)) => assert_eq!(old, new, "{payload:?}"),
            (Err(old), Err(new)) => assert_eq!(old.to_string(), new.to_string()),
            (old, new) => panic!("encoding mismatch: {old:?}, {new:?}"),
        }
    }
    for value in [
        json!(""),
        json!("a +&%?#/中文"),
        json!(true),
        json!(false),
        json!(i64::MIN),
        json!(u64::MAX),
        json!(-0.0),
        json!(1e-7),
        json!(1e20),
        json!(f64::MAX),
        json!(null),
        json!([]),
        json!({"nested":1}),
    ] {
        check(value);
    }
    let mut bits = 1_u64;
    for _ in 0..10000 {
        if let Some(number) = serde_json::Number::from_f64(f64::from_bits(bits)) {
            check(Value::Number(number));
        }
        bits = bits.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    }
    assert_eq!(super::web::query_string(&serde_json::Map::new()).unwrap(), "");
}
#[test]
fn small_glob_backend_preserves_patterns_and_validation() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("src")).unwrap();
    for name in ["src/main.rs", "src/lib.rs", "README.md", "你好.txt"] {
        fs::write(root.path().join(name), "fixture").unwrap();
    }
    for (pattern, expected) in [
        ("**/*.rs", vec!["src/lib.rs", "src/main.rs"]),
        ("src/{main,lib}.rs", vec!["src/lib.rs", "src/main.rs"]),
        ("src/[ml]*.rs", vec!["src/lib.rs", "src/main.rs"]),
        ("你好*.txt", vec!["你好.txt"]),
        ("你?.txt", vec![]), // Preserve upstream byte-oriented wildcard semantics.
        ("你???.txt", vec!["你好.txt"]),
    ] {
        let reply =
            super::search::handle("tool", json!({"name":"find_files","arguments":{"glob":pattern}}), root.path())
                .unwrap();
        let output: Value = serde_json::from_str(reply["output"].as_str().unwrap()).unwrap();
        let paths: Vec<_> = output["matches"].as_array().unwrap().iter().map(|v| v["path"].as_str().unwrap()).collect();
        assert_eq!(paths, expected, "{pattern}");
    }
    assert!(
        super::search::handle("tool", json!({"name":"find_files","arguments":{"glob":"[z-a]"}}), root.path(),).is_err()
    );
}
#[test]
fn small_search_preserves_unicode_and_default_case_folding() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("sample.txt"), "Hello 你好 Σ K\n").unwrap();
    for (query, regex) in [
        ("hello", false),
        ("σ", false),
        ("k", false),
        (r"\p{Script=Han}+", true),
        (r"\b你好\b", true),
        (r"\p{Alphabetic}+", true),
        (r"\p{Age=3.0}+", true),
    ] {
        let reply = super::search::handle(
            "tool",
            json!({"name":"search","arguments":{"query":query,"regex":regex}}),
            root.path(),
        )
        .unwrap();
        let output: Value = serde_json::from_str(reply["output"].as_str().unwrap()).unwrap();
        assert_eq!(output["matches"].as_array().unwrap().len(), 1, "{query}");
        assert_eq!(output["matches"][0]["path"], "sample.txt");
    }
}
#[test]
fn smoke_output_cap_is_explicit_and_bounded() {
    let request = json!({"body":{"max_tokens":4096,"tools":[{}],"tool_choice":"auto"}});
    let default = super::smoke::handle("before_request", request.clone(), &[]).unwrap();
    assert_eq!(default["body"]["max_tokens"], 32);
    let thinking =
        super::smoke::handle("before_request", request.clone(), &["--output-cap".into(), "1024".into()]).unwrap();
    assert_eq!(thinking["body"]["max_tokens"], 1024);
    assert!(thinking["body"].get("tools").is_none() && thinking["body"].get("tool_choice").is_none());
    for invalid in ["0", "1025", "bad"] {
        assert!(
            super::smoke::handle("before_request", request.clone(), &["--output-cap".into(), invalid.into()]).is_err()
        );
    }
}
#[test]
fn logo_is_deterministic_and_keeps_i_visible_without_color() {
    let plain = brand::render(false, true);
    assert!(plain.is_ascii() && !plain.contains('\x1b'));
    assert!(plain.contains("lninl / i") && plain.lines().any(|line| line.contains('o')));
    assert_eq!(plain, brand::render(false, true));
    let rows: Vec<_> = plain.lines().skip(1).take(7).collect();
    assert!(rows[0].starts_with("   ##") && rows[6].starts_with("    ##"));
    assert!(!rows.iter().any(|row| row.contains("#####")), "lowercase l must not use the capital L foot");
    for row in &rows {
        assert_eq!(*row, row.chars().rev().collect::<String>(), "each logo row must mirror around i");
    }
    assert!(brand::render(true, false).contains("\x1b[1;36m"));
    let svg = brand::svg();
    assert!(svg.starts_with("<svg") && svg.contains("#22d3ee") && svg.contains("lninl / i"));
    assert!(!svg.contains('\x1b'));
    assert!(svg.contains("viewBox=\"0 0 343 132\"") && svg.contains("cx=\"171.5\""));
    let pattern = regex::Regex::new(r#"<rect x="(\d+)" y="(\d+)" width="7""#).unwrap();
    let pixels: std::collections::HashSet<(u32, u32)> =
        pattern.captures_iter(&svg).map(|capture| (capture[1].parse().unwrap(), capture[2].parse().unwrap())).collect();
    assert!(!pixels.is_empty());
    assert!(pixels.iter().all(|&(x, y)| pixels.contains(&(336 - x, y))), "SVG pixels must mirror too");
}
use serde_json::{Value, json};
use std::{fs, path::Path};

#[test]
fn cache_usage_normalizes_providers_without_double_counting() {
    for usage in [
        json!({"prompt_tokens":10000,"prompt_tokens_details":{"cached_tokens":9900}}),
        json!({"input_tokens":10000,"input_tokens_details":{"cached_tokens":9900}}),
        json!({"prompt_tokens":10000,"prompt_cache_hit_tokens":9900,"prompt_cache_miss_tokens":100}),
        json!({"input_tokens":100,"cache_read_input_tokens":9900,"cache_creation_input_tokens":0}),
    ] {
        assert_eq!(cache::usage(&usage), Some(cache::Usage { input: 10000, read: 9900, write: 0 }));
    }
    assert_eq!(
        cache::usage(&json!({"input_tokens":500,"cache_read_input_tokens":9000,
        "cache_creation_input_tokens":500,"cache_creation":{"ephemeral_5m_input_tokens":500}})),
        Some(cache::Usage { input: 10000, read: 9000, write: 500 })
    );
    assert_eq!(
        cache::usage(
            &json!({"input_tokens":10000,"input_tokens_details":{"cached_tokens":9000,"cache_write_tokens":500}})
        ),
        Some(cache::Usage { input: 10000, read: 9000, write: 500 })
    );
    for invalid in [
        json!({}),
        json!({"prompt_tokens":10000}),
        json!({"input_tokens":10,"input_tokens_details":{"cached_tokens":11}}),
        json!({"prompt_tokens":100,"prompt_tokens_details":{"cached_tokens":-1}}),
        json!({"prompt_tokens":100,"prompt_cache_hit_tokens":90,"prompt_cache_miss_tokens":11}),
        json!({"prompt_tokens":100,"prompt_cache_hit_tokens":90,"prompt_cache_miss_tokens":null}),
        json!({"prompt_tokens":100,"prompt_cache_hit_tokens":90,"prompt_cache_miss_tokens":-1}),
        json!({"prompt_tokens":100,"prompt_cache_hit_tokens":90,"prompt_tokens_details":{"cached_tokens":89}}),
        json!({"prompt_tokens":u64::MAX,"prompt_cache_hit_tokens":u64::MAX,"prompt_cache_miss_tokens":1}),
    ] {
        assert!(cache::usage(&invalid).is_none());
    }
}
fn request(messages: Value) -> Value {
    json!({"url":"https://provider.invalid/v1/chat/completions","compact":false,
        "history":[{"role":"system","content":"fixed","_cache_session":"session"}],
        "body":{"model":"fixture","messages":messages,"tools":[]}})
}
fn response(request: &Value, input: u64, cached: u64) -> Value {
    json!({"request_meta":request["request_meta"],"response":{"usage":{
        "prompt_tokens":input,"prompt_tokens_details":{"cached_tokens":cached}}}})
}
#[test]
fn cache_reports_real_usage_and_includes_cold_and_missing_requests() {
    let root = tempfile::tempdir().unwrap();
    let first =
        cache::handle("before_request", request(json!([{"role":"user","content":"initial"}])), root.path(), &[])
            .unwrap();
    assert_eq!(first["request_meta"]["cache"]["phase"], "new");
    let reply = response(&first, 10000, 0);
    cache::handle("after_response", reply.clone(), root.path(), &[]).unwrap();
    cache::handle("after_response", reply, root.path(), &[]).unwrap(); // Duplicate delivery is not double-counted.
    let second = cache::handle(
        "before_request",
        request(json!([
        {"role":"user","content":"initial"},{"role":"assistant","content":"next"}])),
        root.path(),
        &[],
    )
    .unwrap();
    assert_eq!(second["request_meta"]["cache"]["phase"], "append");
    cache::handle("after_response", response(&second, 10000, 9900), root.path(), &[]).unwrap();
    let report = cache::report(root.path(), &[]).unwrap();
    assert_eq!(report["totals"]["observed_responses"], 2);
    assert_eq!(report["totals"]["observed_token_hit_ratio"], 0.495);
    assert_eq!(report["totals"]["target_met"], false);
    assert_eq!(report["phases"]["append"]["target_met"], true);
    assert!(!fs::read_to_string(root.path().join("metrics/cache-stats.json")).unwrap().contains("initial"));
    let third =
        cache::handle("before_request", request(json!([{"role":"user","content":"replacement"}])), root.path(), &[])
            .unwrap();
    assert_eq!(third["request_meta"]["cache"]["phase"], "reset");
    let report = cache::report(root.path(), &[]).unwrap();
    assert_eq!(report["totals"]["unobserved_prepared_requests"], 1);
    assert!(report["totals"]["target_met"].is_null());
    cache::handle("after_response", json!({"request_meta":third["request_meta"],"response":{}}), root.path(), &[])
        .unwrap();
    let report = cache::report(root.path(), &[]).unwrap();
    assert_eq!(report["totals"]["unknown_usage_responses"], 1);
    assert!(report["totals"]["target_met"].is_null());
}
#[test]
fn cache_fingerprints_track_settings_and_text_without_rewriting_requests() {
    let original = request(json!([{"role":"user","content":"{ \"z\":1, \"a\":2 }"}]));
    for (change, reason) in [
        ("round-trip", "previous_message_prefix_preserved"),
        ("text", "history_changed"),
        ("setting", "request_settings_changed"),
        ("route", "request_settings_changed"),
    ] {
        let root = tempfile::tempdir().unwrap();
        let first = cache::handle("before_request", original.clone(), root.path(), &[]).unwrap();
        cache::handle("after_response", response(&first, 1000, 0), root.path(), &[]).unwrap();
        let mut next: Value = serde_json::from_str(&serde_json::to_string_pretty(&original).unwrap()).unwrap();
        match change {
            "text" => next["body"]["messages"][0]["content"] = json!("{\"a\":2,\"z\":1}"),
            "setting" => next["body"]["temperature"] = json!(0.2),
            "route" => next["url"] = json!("https://provider.invalid/other/chat/completions"),
            _ => {}
        }
        let prepared = cache::handle("before_request", next.clone(), root.path(), &[]).unwrap();
        assert_eq!(prepared["body"], next["body"]);
        assert_eq!(prepared["request_meta"]["cache"]["reason"], reason);
        assert_eq!(prepared["request_meta"]["cache"]["request_hash"], context::digest(&next["body"]));
        if change == "round-trip" {
            assert_eq!(
                first["request_meta"]["cache"]["request_hash"],
                prepared["request_meta"]["cache"]["request_hash"]
            );
        }
        let mut reply = response(&prepared, 1000, 900);
        reply["response"]["usage"]["prompt_cache_hit_tokens"] = json!(900);
        reply["response"]["usage"]["prompt_cache_miss_tokens"] = json!(101);
        cache::handle("after_response", reply, root.path(), &[]).unwrap();
        let report = cache::report(root.path(), &[]).unwrap();
        assert_eq!(report["totals"]["unknown_usage_responses"], 1);
        assert!(report["totals"]["target_met"].is_null());
        assert!(report["recent"][1]["uncached_input_tokens"].is_null());
        assert_eq!(report["recent"][1]["prefix_diagnostic"], reason);
    }
}
#[test]
fn unchanged_prefix_does_not_imply_a_provider_cache_hit() {
    let root = tempfile::tempdir().unwrap();
    for _ in 0..2 {
        let prepared =
            cache::handle("before_request", request(json!([{"role":"user","content":"same"}])), root.path(), &[])
                .unwrap();
        cache::handle("after_response", response(&prepared, 1000, 0), root.path(), &[]).unwrap();
    }
    let report = cache::report(root.path(), &[]).unwrap();
    assert_eq!(report["phases"]["append"]["observed_token_hit_ratio"], 0.0);
    assert_eq!(report["phases"]["append"]["target_met"], false);
}
#[test]
fn cache_controls_are_explicit_stable_and_preserve_user_overrides() {
    let root = tempfile::tempdir().unwrap();
    let cfg = root.path().join("cache.json");
    let args = vec!["--config".into(), cfg.to_string_lossy().into_owned()];
    fs::write(&cfg, r#"{"mode":"openai","retention":"24h"}"#).unwrap();
    let first = cache::handle("before_request", request(json!([])), root.path(), &args).unwrap();
    let next =
        cache::handle("before_request", request(json!([{"role":"user","content":"new tail"}])), root.path(), &args)
            .unwrap();
    assert_eq!(first["body"]["prompt_cache_key"], next["body"]["prompt_cache_key"]);
    assert_eq!(first["body"]["prompt_cache_key"].as_str().unwrap().len(), 64);
    assert_eq!(next["body"]["prompt_cache_retention"], "24h");
    let mut custom = request(json!([]));
    custom["body"]["prompt_cache_key"] = json!("explicit-key");
    let custom = cache::handle("before_request", custom, root.path(), &args).unwrap();
    assert_eq!(custom["body"]["prompt_cache_key"], "explicit-key");
    fs::write(&cfg, r#"{"mode":"anthropic","ttl":"1h"}"#).unwrap();
    let claude = cache::handle("before_request", request(json!([])), root.path(), &args).unwrap();
    assert_eq!(claude["body"]["cache_control"], json!({"type":"ephemeral","ttl":"1h"}));
    let plain = request(json!([]));
    assert_eq!(cache::handle("before_request", plain.clone(), root.path(), &[]).unwrap()["body"], plain["body"]);
    fs::write(&cfg, r#"{"mode":"guess-provider"}"#).unwrap();
    assert!(cache::handle("before_request", request(json!([])), root.path(), &args).is_err());
}
#[test]
fn reference_updates_append_deltas_and_restore_missing_baselines() {
    let mut data = json!({"messages":[{"role":"system","content":"immutable"},{"role":"user","content":"task"}]});
    context::update(&mut data, "memory", json!({"a":"one","b":"keep"})).unwrap();
    let baseline = data["messages"][2].clone();
    context::update(&mut data, "memory", json!({"a":"two","b":"keep"})).unwrap();
    assert_eq!(data["messages"][0]["content"], "immutable");
    assert_eq!(data["messages"][2], baseline);
    let delta = data["messages"][3]["content"].as_str().unwrap();
    assert!(delta.contains("Kind: delta") && delta.contains("two") && !delta.contains("keep"));
    let unchanged = data.clone();
    context::update(&mut data, "memory", json!({"a":"two","b":"keep"})).unwrap();
    assert_eq!(data, unchanged);
    // A summary may retain the latest delta but discard the full snapshot it depends on.
    data["messages"].as_array_mut().unwrap().remove(2);
    context::update(&mut data, "memory", json!({"a":"two","b":"keep"})).unwrap();
    let rebuilt = data["messages"].as_array().unwrap().last().unwrap();
    assert_eq!(rebuilt["_context_full"], true);
    assert!(rebuilt["content"].as_str().unwrap().contains("keep"));
    context::update(&mut data, "memory", json!({})).unwrap();
    assert!(
        data["messages"].as_array().unwrap().last().unwrap()["content"]
            .as_str()
            .unwrap()
            .contains("\"remove\":[\"a\",\"b\"]")
    );
}
#[test]
fn local_bookkeeping_does_not_invalidate_usage_anchors() {
    let mut history = json!([{"role":"system","content":"fixed","_cache_session":"one"},
        {"role":"assistant","content":"answer","_usage":{"tokens":1}}]);
    let initial = budget::fingerprint(&history);
    history[0]["_cache_session"] = json!("two");
    history[0]["_context_memory"] = json!({"snapshot":{"x":"changed"}});
    history[1]["_usage"] = json!({"tokens":2});
    assert_eq!(budget::fingerprint(&history), initial);
    history[1]["_responses_output"] = json!([{"type":"reasoning","encrypted_content":"opaque"}]);
    assert_ne!(budget::fingerprint(&history), initial);
}
#[test]
fn summary_budget_omits_unadvertised_tools_and_exposes_comparable_estimates() {
    let data = json!({"policy":{"context_window":1000},"model":"fixture","max_tokens":100,
        "history":[{"role":"user","content":"short"}],"tools":"x".repeat(2000),"compact":false});
    let ordinary = budget::budget(data.clone()).unwrap();
    let mut summary = data;
    summary["compact"] = json!(true);
    let summary = budget::budget(summary).unwrap();
    assert_eq!(ordinary["fits"], false);
    assert_eq!(summary["fits"], true);
    assert!(summary["content_estimate_tokens"].as_u64().unwrap() < 100);
}
#[test]
fn platform_paths_keep_persistent_state_out_of_disposable_storage() {
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let mut paths = AppPaths {
        config_dir: home.path().join("config"),
        data_dir: home.path().join("data"),
        cache_dir: home.path().join("cache"),
        runtime_dir: home.path().join("runtime"),
    };
    let (id, store) = paths.workspace(project.path(), None).unwrap();
    assert_eq!(paths.workspace(project.path(), None).unwrap(), (id.clone(), store.clone()));
    assert_eq!(paths.workspace(Path::new("mobile-document-grant"), Some(&id)).unwrap().1, store);
    memory::handle("tool", json!({"name":"memory_set","arguments":{"key":"keep","value":"persistent"}}), &store)
        .unwrap();
    assert!(state::workspace_state(&store, "memory.json").unwrap().exists());
    fs::create_dir_all(&paths.cache_dir).unwrap();
    fs::remove_dir_all(&paths.cache_dir).unwrap();
    assert!(store.join("memory.json").exists());
    let other = tempfile::tempdir().unwrap();
    assert_ne!(paths.workspace(other.path(), None).unwrap().0, id);
    paths.data_dir = paths.cache_dir.join("unsafe");
    assert!(paths.validate().is_err());
    paths.data_dir = home.path().join("cache/../data");
    assert!(paths.validate().is_err());
    #[cfg(unix)]
    {
        paths.data_dir = home.path().join("data");
        std::os::unix::fs::symlink(&paths.data_dir, &paths.cache_dir).unwrap();
        assert!(paths.validate().is_err());
    }
}
#[cfg(unix)]
#[test]
fn default_skill_roots_reject_symlink_escape_but_explicit_roots_work() {
    let outside = tempfile::tempdir().unwrap();
    fs::create_dir(outside.path().join("private")).unwrap();
    fs::write(outside.path().join("private/SKILL.md"), "---\nname: private\ndescription: outside metadata\n---\nbody")
        .unwrap();
    for parent in [".agents", ".i"] {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join(parent)).unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join(parent).join("skills")).unwrap();
        let data = json!({"messages":[{"role":"system","content":"base"}]});
        assert!(skills::handle("before_turn", data.clone(), root.path(), &[]).is_err());
        let config = root.path().join("skills.json");
        fs::write(&config, json!({"roots":[outside.path()]}).to_string()).unwrap();
        let args = vec!["--config".into(), config.to_string_lossy().into_owned()];
        let explicit = skills::handle("before_turn", data, root.path(), &args).unwrap();
        assert!(explicit["messages"][1]["content"].as_str().unwrap().contains("outside metadata"));
    }
}
