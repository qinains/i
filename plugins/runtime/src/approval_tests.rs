use super::*;
use std::fs;

fn action(name: &str, external: bool, decision: &str) -> Value {
    json!({"name":name,"arguments":{},"external":external,"decision":decision})
}
#[test]
fn approval_modes_reload_atomically_and_missing_or_invalid_state_fails_closed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mode.json");
    let args = vec!["--mode-file".into(), path.to_str().unwrap().into()];
    let write = action("write", false, "ask");
    assert!(approval::handle("tool_approval", write.clone(), &args).is_err());
    for (mode, decision) in [("auto", "allow"), ("ask", "ask"), ("auto", "allow")] {
        approval::set_mode(&path, mode).unwrap();
        assert_eq!(approval::mode(&path).unwrap(), mode);
        assert_eq!(approval::handle("tool_approval", write.clone(), &args).unwrap()["decision"], decision);
    }
    for invalid in ["", "garbage", "{}", "{\"mode\":\"on\"}", "{\"mode\":true}"] {
        fs::write(&path, invalid).unwrap();
        assert!(approval::handle("tool_approval", write.clone(), &args).is_err());
    }
    fs::remove_file(&path).unwrap();
    assert!(approval::handle("tool_approval", write, &args).is_err());
    for flag in ["--mode-file", "--config"] {
        assert!(approval::handle("tool_approval", action("write", false, "allow"), &[flag.into()]).is_err());
    }
    assert!(approval::set_mode(&path, "unknown").is_err());
    assert!(!path.exists());
}
#[test]
fn approval_preserves_defaults_but_explicit_policy_can_override_yes_and_deny_in_auto() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("approval.json");
    let args = vec!["--config".into(), file.to_str().unwrap().into()];
    assert_eq!(approval::handle("tool_approval", action("write", false, "allow"), &[]).unwrap()["decision"], "allow");
    fs::write(&file, r#"{"mode":"ask"}"#).unwrap();
    for (name, external, decision) in [("write", false, "ask"), ("read", false, "allow"), ("read", true, "ask")] {
        assert_eq!(
            approval::handle("tool_approval", action(name, external, "allow"), &args).unwrap()["decision"],
            decision
        );
    }
    fs::write(&file, r#"{"mode":"auto","deny_tools":["shell"]}"#).unwrap();
    for (name, default, decision) in [("write", "ask", "allow"), ("shell", "ask", "deny"), ("write", "deny", "deny")] {
        assert_eq!(
            approval::handle("tool_approval", action(name, false, default), &args).unwrap()["decision"],
            decision
        );
    }
    for invalid in
        [r#"{"mode":null}"#, r#"{"mode":"invalid"}"#, r#"{"deny_tools":[1]}"#, r#"{"deny_tool":["shell"]}"#, "[]"]
    {
        fs::write(&file, invalid).unwrap();
        assert!(approval::handle("tool_approval", action("write", false, "ask"), &args).is_err());
    }
    assert!(approval::handle("tool_approval", action("write", false, "invalid"), &[]).is_err());
    assert_eq!(approval::handle("other_event", json!({"unchanged":true}), &[]).unwrap(), json!({"unchanged":true}));
}
