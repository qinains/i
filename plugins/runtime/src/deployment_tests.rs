use super::*;
#[test]
fn install_is_verified_atomic_and_preserves_previous_executable() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let bin = root.path().join("bin");
    std::fs::write(&source, b"first executable").unwrap();
    assert!(install::copy(&source, &bin, false, Some(&"0".repeat(64))).is_err());
    assert!(!bin.exists());
    let target = install::copy(&source, &bin, false, None).unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"first executable");
    assert_eq!(install::copy(&source, &bin, false, None).unwrap(), target);
    std::fs::write(&source, b"second executable").unwrap();
    install::copy(&source, &bin, true, None).unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"second executable");
    let backup = std::fs::read_dir(&bin)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy().contains(".backup-"))
        .unwrap();
    assert_eq!(std::fs::read(backup.path()).unwrap(), b"first executable");
    #[cfg(unix)]
    {
        use std::os::unix::fs::{PermissionsExt, symlink};
        assert_eq!(std::fs::metadata(&target).unwrap().permissions().mode() & 0o777, 0o755);
        std::fs::remove_file(&target).unwrap();
        symlink(&source, &target).unwrap();
        assert!(install::copy(&source, &bin, true, None).is_err());
    }
}
#[test]
fn deployment_options_and_secret_references_fail_closed() {
    assert!(
        options(&["--key".into(), "never-print-this".into()], &["--model"], &[])
            .unwrap_err()
            .to_string()
            .find("never-print-this")
            .is_none()
    );
    assert!(options(&["--model".into()], &["--model"], &[]).is_err());
    assert!(options(&["--replace=true".into()], &[], &["--replace"]).is_err());
    assert!(super::super::deployment_key("I_CREDENTIAL_invalid", String::new()).is_err());
    assert_eq!(super::super::deployment_key("I_CREDENTIAL_invalid", "env-priority".into()).unwrap(), "env-priority");
    assert!(super::super::credentials::store("invalid\nkey").is_err());
}
