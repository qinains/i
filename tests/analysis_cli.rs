#![cfg(feature = "native")]
use serde_json::json;
use std::{fs, process::Command};
mod support;

#[test]
fn cli_runs_local_analysis_without_a_task_or_session_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(temp.path()).unwrap();
    let session = root.join("session.json");
    let manifest = root.join("plugins.json");
    let original=json!({"version":1,"workspace":root,"messages":[{"role":"system","content":"HOST_SYSTEM"},{"role":"user","content":"real input"},{"role":"assistant","content":"NOT_USER_INPUT"}]}).to_string();
    fs::write(&session, &original).unwrap();
    let worker = support::fixture(
        "analysis-filter",
        json!({"denied":["HOST_SYSTEM","NOT_USER_INPUT"],"summary":"offline reflection"}),
    );
    fs::write(
        &manifest,
        json!({"tools":[],"analyses":[{"name":"review","selection":{"roles":["user"]},"exec":worker}]}).to_string(),
    )
    .unwrap();
    for (roles, success) in [(&[][..], true), (&["assistant"][..], false)] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_i"));
        support::isolate(&mut command, &root);
        command.arg("-C").arg(&root).arg("--plugins").arg(&manifest).arg("--session").arg(&session).args([
            "--base-url",
            "http://127.0.0.1:1",
            "--model",
            "fixture",
            "--api-key",
            "FIXTURE_KEY",
            "--analyze",
            "review",
        ]);
        for role in roles {
            command.args(["--analysis-role", role]);
        }
        let output = command.output().unwrap();
        assert_eq!(output.status.success(), success, "{}", String::from_utf8_lossy(&output.stderr));
        if success {
            assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "offline reflection");
        }
        assert_eq!(fs::read_to_string(&session).unwrap(), original);
        assert!(!root.join("session.precompact.json").exists());
    }
}
