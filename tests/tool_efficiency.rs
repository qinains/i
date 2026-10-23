use i::{
    platform::{FileSystem, native::NativeFs},
    workspace::{Limits, Workspace},
};
use serde_json::{Value, json};
use std::path::Path;

fn result(value: Value) -> Value {
    serde_json::from_str(value["output"].as_str().unwrap()).unwrap()
}
fn workspace(root: &tempfile::TempDir) -> Workspace<NativeFs> {
    Workspace::new(NativeFs::new(root.path().into()).unwrap(), Limits::browser())
}

#[tokio::test]
async fn glob_locates_nested_files_without_directory_round_trips() {
    let root = tempfile::tempdir().unwrap();
    let ws = workspace(&root);
    for p in ["a.rs", "src/b.rs", "src/c.js", "node_modules/ignored.rs"] {
        ws.fs.write_file(Path::new(p), b"x").await.unwrap();
    }
    let first = result(ws.execute("glob", &json!({"patterns":["**/*.rs"],"limit":1})).await.unwrap());
    assert_eq!(first["results"], json!(["a.rs"]));
    assert_eq!(first["truncated"], true);
    let next = result(ws.execute("glob", &json!({"patterns":["**/*.rs"],"cursor":first["next"]})).await.unwrap());
    assert_eq!(next["results"], json!(["src/b.rs"]));
    assert_eq!(next["truncated"], false);
}

#[tokio::test]
async fn search_returns_targeted_context_or_only_matching_filenames() {
    let root = tempfile::tempdir().unwrap();
    let ws = workspace(&root);
    ws.fs.write_file(Path::new("src/a.txt"), "before\n你好 needle\nafter\nNEEDLE\n".as_bytes()).await.unwrap();
    ws.fs.write_file(Path::new("binary"), &[0, 255]).await.unwrap();
    let found = result(
        ws.execute("search", &json!({"pattern":"needle","case_sensitive":false,"context_lines":1,"limit":1}))
            .await
            .unwrap(),
    );
    assert_eq!(found["results"][0]["line"], 2);
    assert_eq!(found["results"][0]["before"], json!(["before"]));
    assert_eq!(found["truncated"], true);
    let next = result(
        ws.execute("search", &json!({"pattern":"needle","case_sensitive":false,"cursor":found["next"]})).await.unwrap(),
    );
    assert_eq!(next["results"][0]["line"], 4);
    assert_eq!(next["truncated"], false);
    let files =
        result(ws.execute("search", &json!({"pattern":"needle","case_sensitive":false,"mode":"files"})).await.unwrap());
    assert_eq!(files["results"], json!(["src/a.txt"]));
    let regex = result(ws.execute("search", &json!({"pattern":"N[A-Z]+","regex":true})).await.unwrap());
    assert_eq!(regex["results"][0]["line"], 4);
    assert!(ws.execute("search", &json!({"pattern":"[","regex":true})).await.is_err());
}

#[tokio::test]
async fn search_snippet_centers_on_match_in_long_unicode_line() {
    let root = tempfile::tempdir().unwrap();
    let ws = workspace(&root);
    ws.fs
        .write_file(Path::new("long"), format!("{} needle {}", "中文".repeat(1000), "尾".repeat(1000)).as_bytes())
        .await
        .unwrap();
    let page = result(ws.execute("search", &json!({"pattern":"needle"})).await.unwrap());
    assert!(page["results"][0]["text"].as_str().unwrap().contains("needle"));
    assert!(page["results"][0]["text"].as_str().unwrap().len() < 450);
}

#[tokio::test]
async fn read_batches_ranges_and_keeps_success_when_one_file_is_missing() {
    let root = tempfile::tempdir().unwrap();
    let ws = workspace(&root);
    ws.fs.write_file(Path::new("a"), b"one\ntwo\nthree\n").await.unwrap();
    let read = result(
        ws.execute(
            "read",
            &json!({"files":[{"path":"a","start_line":2,"end_line":2},{"path":"missing"},{"path":"a","start_line":3}]}),
        )
        .await
        .unwrap(),
    );
    assert_eq!(read["results"][0]["content"], "2: two\n");
    assert!(read["results"][1]["error"].is_string());
    assert_eq!(read["results"][2]["content"], "3: three\n");
    assert!(ws.execute("read", &json!({"path":"a"})).await.is_err());
}

#[tokio::test]
async fn edits_validate_all_replacements_before_a_single_write() {
    let root = tempfile::tempdir().unwrap();
    let ws = workspace(&root);
    ws.fs.write_file(Path::new("a"), b"one\r\ntwo\r\nthree\r\n").await.unwrap();
    assert!(
        ws.execute("edit", &json!({"path":"a","edits":[{"old":"one","new":"ONE"},{"old":"missing","new":"x"}]}))
            .await
            .is_err()
    );
    assert_eq!(ws.fs.read_file(Path::new("a")).await.unwrap(), b"one\r\ntwo\r\nthree\r\n");
    ws.execute(
        "edit",
        &json!({"path":"a","edits":[{"old":"one","new":"ONE"},{"old":"two","new":"TWO","line_number":2}]}),
    )
    .await
    .unwrap();
    assert_eq!(ws.fs.read_file(Path::new("a")).await.unwrap(), b"ONE\r\nTWO\r\nthree\r\n");
}

#[tokio::test]
async fn output_budget_is_shared_and_continuation_is_explicit() {
    let root = tempfile::tempdir().unwrap();
    let ws = workspace(&root);
    for n in 0..20 {
        ws.fs
            .write_file(Path::new(&format!("{n:02}.txt")), format!("{}\n", "needle 中文".repeat(100)).as_bytes())
            .await
            .unwrap();
    }
    for (name, args) in [
        ("search", json!({"pattern":"needle","max_bytes":1024})),
        ("read", json!({"files":[{"path":"00.txt"},{"path":"01.txt"}],"max_bytes":1024})),
    ] {
        let value = ws.execute(name, &args).await.unwrap();
        assert!(value["output"].as_str().unwrap().len() <= 1024);
        let page = result(value);
        assert_eq!(page["truncated"], true);
        assert!(!page["next"].is_null());
    }
    assert!(ws.execute("glob", &json!({"patterns":["**/*"],"path":"../"})).await.is_err());
    assert!(ws.execute("read", &json!({"files":[{"path":"00.txt","start_line":0}]})).await.is_err());
}
