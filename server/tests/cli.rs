use std::process::Command;

fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_i-relay"));
    command.env_clear();
    if let Some(root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", root);
    }
    command
}

#[test]
fn help_and_version_work_without_credentials() {
    for argument in ["--help", "--version"] {
        let result = command().arg(argument).output().unwrap();
        assert!(result.status.success());
        assert!(String::from_utf8_lossy(&result.stdout).contains("i-relay"));
    }
}

#[test]
fn errors_never_echo_environment_credentials_or_unknown_arguments() {
    let fake = "FIXTURE_ONLY_DO_NOT_LOG_CREDENTIAL_0123456789";
    let result = command().env("HARNESS_RELAY_TOKEN", fake).output().unwrap();
    assert!(!result.status.success());
    let error = String::from_utf8_lossy(&result.stderr);
    assert!(error.contains("HARNESS_PUBLIC_ORIGIN"));
    assert!(!error.contains(fake));
    let result = command().arg(fake).output().unwrap();
    assert!(!result.status.success());
    assert!(!String::from_utf8_lossy(&result.stderr).contains(fake));
}
