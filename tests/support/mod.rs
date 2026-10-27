// Native test workers implement the production NDJSON framing contract.
#![allow(dead_code)]
use std::{path::Path, process::Command};
pub fn fixture(mode: &str, config: serde_json::Value) -> serde_json::Value {
    let binary = std::env::var_os("I_TEST_FIXTURE_BIN").map(std::path::PathBuf::from).unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target/debug")
            .join(format!("i-test-fixtures{}", std::env::consts::EXE_SUFFIX))
    });
    assert!(
        binary.is_absolute() && binary.is_file(),
        "build the Rust fixtures first: cargo build --manifest-path tools/test-fixtures/Cargo.toml --target-dir target (or set absolute I_TEST_FIXTURE_BIN)"
    );
    serde_json::json!([binary, mode, config.to_string()])
}
pub fn isolate(command: &mut Command, root: &Path) {
    for key in [
        "API_KEY",
        "DEEPSEEK_API_KEY",
        "DEEPSEEK_TEST_API_KEY",
        "OPENAI_API_KEY",
        "I_TEST_UNSET_KEY",
        "HTTP_PROXY",
        "http_proxy",
        "HTTPS_PROXY",
        "https_proxy",
        "ALL_PROXY",
        "all_proxy",
        "NO_PROXY",
        "no_proxy",
    ] {
        command.env_remove(key);
    }
    for (key, name) in
        [("I_CONFIG_DIR", "config"), ("I_DATA_DIR", "data"), ("I_CACHE_DIR", "cache"), ("I_RUNTIME_DIR", "runtime")]
    {
        command.env(key, root.join(".app").join(name));
    }
}
pub fn store(root: &Path) -> std::path::PathBuf {
    let path = root.join(".app/data/workspaces");
    std::fs::read_dir(path).unwrap().next().unwrap().unwrap().path()
}
