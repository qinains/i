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
