#[test]
fn native_file_roundtrip() {
    use i::platform::{FileSystem, native::NativeFs};
    let root = tempfile::tempdir().unwrap();
    let fs = NativeFs::new(root.path().to_path_buf()).unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        fs.write_file(std::path::Path::new("nested/file.bin"), &[0, 255, 10]).await.unwrap();
        assert_eq!(fs.read_file(std::path::Path::new("nested/file.bin")).await.unwrap(), [0, 255, 10]);
    });
}

#[cfg(unix)]
#[test]
fn archive_output_alias_cannot_overwrite_source() {
    use i::platform::native::NativeFs;
    let dir = tempfile::tempdir().unwrap();
    let fs = NativeFs::new(dir.path().into()).unwrap();
    std::fs::create_dir_all(dir.path().join(".i/downloads")).unwrap();
    std::fs::write(dir.path().join("source.txt"), b"original").unwrap();
    let output = dir.path().join(".i/downloads/out.zip");
    std::os::unix::fs::symlink(dir.path().join("source.txt"), &output).unwrap();
    assert!(fs.write_archive(std::path::Path::new(".i/downloads/out.zip"), b"ZIP").is_err());
    assert_eq!(std::fs::read(dir.path().join("source.txt")).unwrap(), b"original");
    std::fs::remove_file(&output).unwrap();
    std::fs::hard_link(dir.path().join("source.txt"), &output).unwrap();
    fs.write_archive(std::path::Path::new(".i/downloads/out.zip"), b"ZIP").unwrap();
    assert_eq!(std::fs::read(dir.path().join("source.txt")).unwrap(), b"original");
    assert_eq!(std::fs::read(output).unwrap(), b"ZIP");
}
