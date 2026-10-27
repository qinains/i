use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn worker_keeps_state_and_echoes_request_ids() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_i-test-fixtures"))
        .args(["pool", "{}"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    writeln!(input, "{}", json!({"id":41,"data":{}})).unwrap();
    writeln!(input, "{}", json!({"id":42,"data":{}})).unwrap();
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let values: Vec<Value> =
        String::from_utf8(output.stdout).unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert_eq!(values[0]["id"], 41);
    assert_eq!(values[1]["id"], 42);
    assert_eq!(values[0]["data"]["pid"], values[1]["data"]["pid"]);
    assert_eq!(values[1]["data"]["n"], 2);
}
