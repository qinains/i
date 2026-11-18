#![cfg(feature = "packaging")]
use std::{fs, process::Command};

#[test]
fn packager_rejects_an_executable_without_the_bundled_runtime() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("Cargo.toml"), "[package]\nversion = \"0.0.0\"\n").unwrap();
    let binaries = root.path().join("target/min-size");
    fs::create_dir_all(&binaries).unwrap();
    let tool = binaries.join(format!("i-package{}", std::env::consts::EXE_SUFFIX));
    fs::copy(env!("CARGO_BIN_EXE_i-package"), &tool).unwrap();
    // A real test executable rejects --runtime instead of implementing it.
    fs::copy(std::env::current_exe().unwrap(), binaries.join(format!("i{}", std::env::consts::EXE_SUFFIX))).unwrap();
    let output = Command::new(tool).arg("--skip-build").output().unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("bundled runtime verification failed"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
