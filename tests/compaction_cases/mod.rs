use super::*;

#[test]
fn bound_replacement_compactor_can_return_local_summary_without_model_summary_call() {
    let dir = tempfile::tempdir().unwrap();
    let session = seed_history(dir.path());
    let script = dir.path().join("replacement.py");
    fs::write(
        &script,
        support::python(
            r#"import json,sys,os
from pathlib import Path
e=json.load(sys.stdin);d=e['data'];event=e['event'];root=Path(e['workspace'])
with (root/'replacement.log').open('a') as f: f.write(json.dumps([event,os.getpid()])+'\n')
if event=='compact_plan':
    d['cut']=4;d['state']={'chosen':'replacement'}
if event=='before_compact':
    assert d['state']['chosen']=='replacement'
    assert d['cut']==4 and len(d['messages'])==3
    d['summary']='LOCAL REPLACEMENT SUMMARY';d['retained_images']=[];d['state']['prepared']=True
if event=='after_compact':
    assert d['state']['prepared'];d['summary']+=' [custom postprocessing]'
json.dump(d,sys.stdout)
"#,
        ),
    )
    .unwrap();
    let python = Command::new("python3").args(["-c", "import sys; print(sys.executable)"]).output().unwrap();
    assert!(python.status.success());
    let python = String::from_utf8(python.stdout).unwrap().trim().to_owned();
    let settings = dir.path().join(".app/config");
    fs::create_dir_all(&settings).unwrap();
    fs::write(settings.join("bindings.json"), json!({"compact":{"exec":[python,script]}}).to_string()).unwrap();
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
        let script = format!(
            r#"import json,sys
from pathlib import Path
e=json.load(sys.stdin);d=e['data'];mode={mode:?}
if e['event']=='context_budget':
    if mode=='missingbaseline': d.pop('content_estimate_tokens',None)
    if mode=='unknownfits': d.pop('fits',None)
if e['event']=='compact_plan': d['cut']={{'zero':0,'outside':999,'tool':3,'string':'4'}}.get(mode,4)
if e['event']=='before_compact':
    if mode=='fail': raise RuntimeError('replacement failed, do not retry')
    d['summary']={{'empty':'','object':{{'tool_calls':[{{'name':'write'}}]}},'expand':'x'*20000,'over_budget':'x'*50000}}.get(mode,'local')
json.dump(d,sys.stdout)
"#
        );
        fs::write(dir.path().join("replacement.py"), support::python(&script)).unwrap();
        let hooks = json!([{"events":["context_budget","before_request"],"exec":[plugin_binary(),"budget"]},
            {"events":["context_budget","compact_plan","before_compact","after_compact"],"exec":["python3","replacement.py"]}]);
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
