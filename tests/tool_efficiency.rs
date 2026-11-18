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

struct ObservedFs {
    inner: NativeFs,
    reads: std::cell::Cell<usize>,
    writes: std::cell::Cell<usize>,
    mutate_on_second_read: bool,
}
impl FileSystem for ObservedFs {
    async fn read_file_limited(&self, p: &Path, limit: usize) -> anyhow::Result<Vec<u8>> {
        let reads = self.reads.get() + 1;
        self.reads.set(reads);
        if self.mutate_on_second_read && reads == 2 {
            self.inner.write_file(p, b"concurrent update").await?;
        }
        self.inner.read_file_limited(p, limit).await
    }
    async fn write_file(&self, p: &Path, content: &[u8]) -> anyhow::Result<()> {
        self.writes.set(self.writes.get() + 1);
        self.inner.write_file(p, content).await
    }
    async fn create_dir_all(&self, p: &Path) -> anyhow::Result<()> {
        self.inner.create_dir_all(p).await
    }
    async fn remove_file(&self, p: &Path) -> anyhow::Result<()> {
        self.inner.remove_file(p).await
    }
    async fn read_dir(&self, p: &Path) -> anyhow::Result<Vec<std::path::PathBuf>> {
        self.inner.read_dir(p).await
    }
    async fn metadata(&self, p: &Path) -> anyhow::Result<Option<i::platform::FileMetadata>> {
        self.inner.metadata(p).await
    }
    async fn canonicalize(&self, p: &Path) -> anyhow::Result<std::path::PathBuf> {
        self.inner.canonicalize(p).await
    }
}

#[tokio::test]
async fn reads_reuse_one_snapshot_and_edits_detect_concurrent_changes() {
    let root = tempfile::tempdir().unwrap();
    let fs = ObservedFs {
        inner: NativeFs::new(root.path().into()).unwrap(),
        reads: 0.into(),
        writes: 0.into(),
        mutate_on_second_read: false,
    };
    fs.inner.write_file(Path::new("a"), b"one\ntwo\n").await.unwrap();
    let ws = Workspace::new(fs, Limits::browser());
    ws.execute("read", &json!({"files":[{"path":"a","end_line":1},{"path":"./a","start_line":2}]})).await.unwrap();
    assert_eq!(ws.fs.reads.get(), 1);
    assert_eq!(ws.fs.writes.get(), 0);

    let mut fs = ws.fs;
    fs.reads.set(0);
    fs.mutate_on_second_read = true;
    let ws = Workspace::new(fs, Limits::browser());
    let error = ws.execute("edit", &json!({"path":"a","edits":[{"old":"one","new":"ONE"}]})).await.unwrap_err();
    assert!(error.to_string().contains("file changed during edit"));
    assert_eq!(ws.fs.writes.get(), 0);
    assert_eq!(ws.fs.inner.read_file(Path::new("a")).await.unwrap(), b"concurrent update");
}

#[tokio::test]
async fn invalid_batch_ranges_reject_before_any_io_and_edits_never_partially_write() {
    let root = tempfile::tempdir().unwrap();
    let fs = ObservedFs {
        inner: NativeFs::new(root.path().into()).unwrap(),
        reads: 0.into(),
        writes: 0.into(),
        mutate_on_second_read: false,
    };
    fs.inner.write_file(Path::new("a"), b"old\n").await.unwrap();
    let ws = Workspace::new(fs, Limits { file_bytes: 1024, write_bytes: 8, output_bytes: 12000 });
    for bad in [
        json!({"path":"../escape"}),
        json!({"path":"a","start_line":0}),
        json!({"path":"a","end_line":1001}),
        json!({"path":"a","start_line":3,"end_line":2}),
    ] {
        assert!(ws.execute("read", &json!({"files":[{"path":"a"},bad]})).await.is_err());
    }
    assert_eq!(ws.fs.reads.get(), 0);
    for edits in [
        json!([]),
        json!([{"old":"","new":"x"}]),
        json!([{"old":"old","new":"too large replacement"}]),
        json!([{"old":"old","new":"OLD"},{"old":"missing","new":"x"}]),
        json!([{"old":"old","new":"x","line_number":0}]),
        json!(vec![json!({"old":"old","new":"old"}); 65]),
    ] {
        assert!(ws.execute("edit", &json!({"path":"a","edits":edits})).await.is_err());
    }
    assert_eq!(ws.fs.writes.get(), 0);
    assert_eq!(ws.fs.inner.read_file(Path::new("a")).await.unwrap(), b"old\n");
}

#[tokio::test]
async fn oversized_read_line_reports_error_instead_of_silent_clipping() {
    let root = tempfile::tempdir().unwrap();
    let ws = workspace(&root);
    ws.fs.write_file(Path::new("a"), "中".repeat(1000).as_bytes()).await.unwrap();
    let value = ws.execute("read", &json!({"files":[{"path":"a"}],"max_bytes":1024})).await.unwrap();
    assert!(value["output"].as_str().unwrap().len() <= 1024);
    let page = result(value);
    assert!(page["results"][0]["error"].as_str().unwrap().contains("line exceeds output budget"));
    assert_eq!(page["next"], json!({"file_index":0,"start_line":1}));
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
    assert!(ws.execute("read", &json!({"path":"a","offset":2,"limit":1})).await.is_err());
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

#[tokio::test]
async fn list_and_read_continuations_resume_without_duplicates() {
    let root = tempfile::tempdir().unwrap();
    let ws = workspace(&root);
    let content = (1..=20).map(|n| format!("line {n}: {}\n", "x".repeat(100))).collect::<String>();
    ws.fs.write_file(Path::new("a"), content.as_bytes()).await.unwrap();
    ws.fs.write_file(Path::new("b"), b"second").await.unwrap();
    let first = result(ws.execute("list", &json!({"limit":1})).await.unwrap());
    let second = result(ws.execute("list", &json!({"cursor":first["next"]})).await.unwrap());
    assert_eq!(first["results"][0]["path"], "a");
    assert_eq!(second["results"][0]["path"], "b");
    assert_eq!(second["truncated"], false);
    let mut start = 1;
    let mut combined = String::new();
    loop {
        let page = result(
            ws.execute("read", &json!({"files":[{"path":"a","start_line":start,"end_line":20}],"max_bytes":1024}))
                .await
                .unwrap(),
        );
        for record in page["results"].as_array().unwrap() {
            combined.push_str(record["content"].as_str().unwrap());
        }
        if page["next"].is_null() {
            break;
        }
        let next = page["next"]["start_line"].as_u64().unwrap();
        assert!(next > start);
        start = next;
    }
    let expected = content.lines().enumerate().map(|(n, line)| format!("{}: {line}\n", n + 1)).collect::<String>();
    assert_eq!(combined, expected);
}

#[tokio::test]
async fn discovery_ignored_roots_and_search_files_cursor_are_consistent() {
    let root = tempfile::tempdir().unwrap();
    let ws = workspace(&root);
    for path in [".i/a.txt", "src/b.txt", "src/c.txt"] {
        ws.fs.write_file(Path::new(path), b"needle\nneedle\n").await.unwrap();
    }
    let hidden = result(ws.execute("glob", &json!({"patterns":["**/*.txt"],"path":".i"})).await.unwrap());
    assert_eq!(hidden["results"], json!([".i/a.txt"]));
    let first = result(ws.execute("search", &json!({"pattern":"needle","mode":"files","limit":1})).await.unwrap());
    let next =
        result(ws.execute("search", &json!({"pattern":"needle","mode":"files","cursor":first["next"]})).await.unwrap());
    assert_eq!(first["results"], json!(["src/b.txt"]));
    assert_eq!(next["results"], json!(["src/c.txt"]));
}

#[test]
fn targeted_replacement_preserves_mixed_line_endings() {
    use i::workspace::text::replace_once;
    assert_eq!(replace_once("a\r\nb\nc\r\n", "b", "B", Some(2)).unwrap(), "a\r\nB\nc\r\n");
    assert!(replace_once("a\nb\n", "a\nb", "x", Some(1)).is_err());
}
