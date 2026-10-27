#[cfg(unix)]
#[test]
fn slash_menu_pty_preserves_explicit_execution_and_paste_boundaries() {
    use std::{
        path::{Path, PathBuf},
        process::Command,
    };
    let fixture = std::env::var_os("I_TEST_FIXTURE_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/i-test-fixtures"));
    assert!(
        fixture.is_absolute() && fixture.is_file(),
        "build the native PTY helper first: cargo build --manifest-path tools/test-fixtures/Cargo.toml --target-dir target"
    );
    let output = Command::new(fixture).args(["console-menu-pty", env!("CARGO_BIN_EXE_i-plugins")]).output().unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("PASS: slash menu"));
}
