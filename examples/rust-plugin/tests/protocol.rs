use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn invoke(args: &[&str], events: &[Value]) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_i-plugin-example"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    for event in events {
        writeln!(input, "{event}").unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let replies: Vec<Value> =
        String::from_utf8(output.stdout).unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert_eq!(replies.len(), events.len());
    for (reply, event) in replies.iter().zip(events) {
        assert_eq!(reply["id"], event["id"]);
        assert!(reply.get("error").is_none(), "{reply}");
    }
    replies
}

#[test]
fn word_count_uses_language_neutral_unicode_tool_contract() {
    let result = invoke(
        &[],
        &[
            json!({"id":1,"event":"tool","workspace":".","data":{"name":"rust_word_count","arguments":{"text":"hello 世界"}}}),
        ],
    );
    let count: Value = serde_json::from_str(result[0]["data"]["output"].as_str().unwrap()).unwrap();
    assert_eq!(count, json!({"words":2,"characters":8}));
}

#[test]
fn replacement_compactor_uses_persistent_worker_protocol() {
    let history: Vec<Value> = ["system", "user", "assistant", "tool", "assistant", "user", "assistant", "user"]
        .iter()
        .map(|role| json!({"role":role,"content":"中文 ".repeat(500)}))
        .collect();
    let result = invoke(
        &["--compact"],
        &[
            json!({"id":1,"event":"compact_plan","data":{"history":history,"compact":true}}),
            json!({"id":2,"event":"before_compact","data":{"messages":history[1..4]}}),
            json!({"id":3,"event":"after_compact","data":{"summary":"preserve this"}}),
        ],
    );
    assert_eq!(result.iter().map(|v| v["id"].as_u64().unwrap()).collect::<Vec<_>>(), [1, 2, 3]);
    assert_eq!(result[0]["data"]["cut"], 4);
    let summary = result[1]["data"]["summary"].as_str().unwrap();
    assert!(summary.len() <= 2048 && summary.contains("中文"));
    assert_eq!(result[2]["data"]["summary"], "preserve this");
}

#[test]
fn hooks_and_unknown_events_preserve_contract() {
    let result = invoke(
        &[],
        &[
            json!({"id":1,"event":"system_prompt","workspace":".","data":{"prompt":"base"}}),
            json!({"id":2,"event":"future_event","workspace":".","data":{"prompt":"base"}}),
        ],
    );
    assert!(result[0]["data"]["prompt"].as_str().unwrap().contains("explicit test results"));
    assert_eq!(result[1]["data"], json!({"prompt":"base"}));
}
