use i::{platform::{FileSystem, native::NativeFs}, workspace::{Workspace, Limits, archive::{DownloadRequest, ArchiveLimits}}};
use std::{io::{Cursor, Read}, path::Path};

#[tokio::test]
async fn zip_roundtrip() {
    let root = tempfile::tempdir().unwrap();
    let ws = Workspace::new(NativeFs::new(root.path().into()).unwrap(), Limits::browser());
    ws.fs.write_file(Path::new("dir/中文.bin"), &[0, 255, 10]).await.unwrap();
    ws.fs.create_dir_all(Path::new("empty")).await.unwrap();
    let req = DownloadRequest::default();
    let zip = ws.archive(&req).await.unwrap();
    assert_eq!(zip.bytes, ws.archive(&req).await.unwrap().bytes);
    let mut reader = zip::ZipArchive::new(Cursor::new(zip.bytes)).unwrap();
    assert_eq!(reader.len(), 3);
    assert!(reader.by_name("empty/").unwrap().is_dir());
    let mut actual = vec![];
    reader.by_name("dir/中文.bin").unwrap().read_to_end(&mut actual).unwrap();
    assert_eq!(actual, [0, 255, 10]);
    let req = DownloadRequest { paths: Some(vec!["dir".into(), "dir/中文.bin".into()]), filename: None };
    assert_eq!(ws.archive(&req).await.unwrap().entries, 2);
}
#[tokio::test]
async fn archive_limits_and_paths() {
    let root = tempfile::tempdir().unwrap();
    let ws = Workspace::new(NativeFs::new(root.path().into()).unwrap(), Limits::browser());
    ws.fs.write_file(Path::new("a"), b"abc").await.unwrap();
    assert!(ws.archive_with_limits(&DownloadRequest::default(), ArchiveLimits { bytes: 2, entries: 10 }).await.is_err());
    assert!(ws.archive_with_limits(&DownloadRequest::default(), ArchiveLimits { bytes: 3, entries: 0 }).await.is_err());
    assert!(ws.archive_with_limits(&DownloadRequest::default(), ArchiveLimits { bytes: 3, entries: 1 }).await.is_ok());
    for filename in ["../x.zip", "a/b.zip", "test.txt", "", "C:x.zip"] {
        assert!(ws.archive(&DownloadRequest { paths: None, filename: Some(filename.into()) }).await.is_err());
    }
    for paths in [vec![], vec!["missing".into()], vec!["../bad".into()]] {
        assert!(ws.archive(&DownloadRequest { paths: Some(paths), filename: None }).await.is_err());
    }
}
#[tokio::test]
async fn native_default_excludes_generated_archives() {
    let root = tempfile::tempdir().unwrap();
    let mut ws = Workspace::new(NativeFs::new(root.path().into()).unwrap(), Limits::browser());
    ws.exclude_downloads = true;
    ws.fs.write_file(Path::new(".i/downloads/old.zip"), b"old").await.unwrap();
    ws.fs.write_file(Path::new("result.txt"), b"yes").await.unwrap();
    let zip = ws.archive(&DownloadRequest::default()).await.unwrap();
    let mut reader = zip::ZipArchive::new(Cursor::new(zip.bytes)).unwrap();
    assert!(reader.by_name("result.txt").is_ok());
    assert!(reader.by_name(".i/downloads/old.zip").is_err());
}
