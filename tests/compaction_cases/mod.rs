use super::*;

#[test]
fn bound_replacement_compactor_can_return_local_summary_without_model_summary_call() {
    let dir = tempfile::tempdir().unwrap();
    let session = seed_history(dir.path());
    let settings = dir.path().join(".app/config");
    fs::create_dir_all(&settings).unwrap();
    fs::write(
        settings.join("bindings.json"),
        json!({"compact":{"exec":support::fixture("replacement-compact",json!({}))}}).to_string(),
    )
    .unwrap();
    let mut command = Command::new(plugin_binary());
    support::isolate(&mut command, dir.path());
    let manifest = command.args(["manifest", "compact"]).output().unwrap();
    assert!(manifest.status.success(), "{}", String::from_utf8_lossy(&manifest.stderr));
    fs::write(dir.path().join("hooks.json"), manifest.stdout).unwrap();
    let config = dir.path().join("api.json");
    fs::write(&config, json!({"network_retries":0,"plugins":"hooks.json","budget":test_budget("omit")}).to_string())
        .unwrap();
    let (base, worker) = server(vec![(200, final_reply())]);
    let result = run(
        dir.path(),
        &base,
        &["--config", config.to_str().unwrap(), "--session", session.to_str().unwrap(), "continue"],
    );
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let requests = worker.join().unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].get("tools").is_some());
    assert!(requests[0].to_string().contains("LOCAL REPLACEMENT SUMMARY [custom postprocessing]"));
    assert!(!requests[0].to_string().contains("data:image"));
    let saved: Value = serde_json::from_slice(&fs::read(&session).unwrap()).unwrap();
    let backup: Value = serde_json::from_slice(&fs::read(session.with_extension("precompact.json")).unwrap()).unwrap();
    assert_eq!(saved["messages"][0], backup["messages"][0]);
    assert_eq!(requests[0]["messages"][0], backup["messages"][0]);
    assert_eq!(backup["messages"][3]["tool_call_id"], "old_call");
    let events: Vec<Value> = fs::read_to_string(dir.path().join("replacement.log"))
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(
        events.iter().map(|v| v[0].as_str().unwrap()).collect::<Vec<_>>(),
        ["compact_plan", "before_compact", "after_compact"]
    );
    assert!(events.iter().all(|v| v[1] == events[0][1]), "replacement worker should be reused");
}

#[test]
fn replacement_failures_and_invalid_candidates_never_fall_back_or_commit() {
    for mode in [
        "zero",
        "outside",
        "tool",
        "string",
        "empty",
        "object",
        "expand",
        "over_budget",
        "fail",
        "missingbaseline",
        "unknownfits",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let session = seed_history(dir.path());
        let hooks = json!([{"events":["context_budget","before_request"],"exec":[plugin_binary(),"budget"]},
            {"events":["context_budget","compact_plan","before_compact","after_compact"],"exec":support::fixture("invalid-compact",json!({"mode":mode}))}]);
        fs::write(dir.path().join("hooks.json"), json!({"tools":[],"hooks":hooks}).to_string()).unwrap();
        let config = dir.path().join("api.json");
        fs::write(
            &config,
            json!({"network_retries":0,"plugins":"hooks.json","budget":test_budget("omit")}).to_string(),
        )
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let result = run(
            dir.path(),
            &format!("http://{}/v1", listener.local_addr().unwrap()),
            &["--config", config.to_str().unwrap(), "--session", session.to_str().unwrap(), "continue"],
        );
        assert!(!result.status.success(), "{mode}");
        let error = String::from_utf8_lossy(&result.stderr);
        let expected = match mode {
            "zero" | "outside" => "invalid compaction cut",
            "tool" => "splits tool batch",
            "string" => "cut",
            "empty" => "empty compaction summary",
            "object" => "summary",
            "expand" => "did not reduce",
            "over_budget" => "still exceeds",
            "fail" => "replacement failed",
            "missingbaseline" => "missing compaction baseline",
            "unknownfits" => "valid context_budget output",
            _ => unreachable!(),
        };
        assert!(error.contains(expected), "{mode}: {error}");
        assert!(
            matches!(listener.accept(),Err(e) if e.kind()==std::io::ErrorKind::WouldBlock),
            "unexpected HTTP in {mode}"
        );
        assert!(!session.with_extension("precompact.json").exists());
        let saved: Value = serde_json::from_slice(&fs::read(&session).unwrap()).unwrap();
        assert_eq!(saved["messages"][3]["tool_call_id"], "old_call");
        assert!(saved.to_string().contains("data:image/png"));
        assert_eq!(saved["messages"].as_array().unwrap().last().unwrap()["content"], "continue");
    }
}

#[test]
fn budget_without_compactor_does_not_implicitly_compress() {
    let dir = tempfile::tempdir().unwrap();
    let session = seed_history(dir.path());
    let config = dir.path().join("api.json");
    fs::write(
        &config,
        json!({"plugins":format!("{}/plugins/budget.json",env!("CARGO_MANIFEST_DIR")),"budget":test_budget("omit")})
            .to_string(),
    )
    .unwrap();
    let (base, worker) = server(vec![(200, final_reply())]);
    let result = run(
        dir.path(),
        &base,
        &["--config", config.to_str().unwrap(), "--session", session.to_str().unwrap(), "continue"],
    );
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let requests = worker.join().unwrap();
    assert!(requests[0].get("tools").is_some());
    assert!(requests[0].to_string().contains("old output"));
    assert!(!session.with_extension("precompact.json").exists());
    let saved: Value = serde_json::from_slice(&fs::read(session).unwrap()).unwrap();
    assert_eq!(saved["messages"][3]["tool_call_id"], "old_call");
}
