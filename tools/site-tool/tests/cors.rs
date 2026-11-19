use i_site_tool::{brand, copy_tree, cors, hash, read_json, site};
use std::{
    fs,
    io::{Cursor, Read},
    path::Path,
};
fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn fixture(path: &Path) {
    copy_tree(&root().join("extensions"), &path.join("extensions")).unwrap();
    fs::write(path.join("LICENSE"), fs::read(root().join("LICENSE")).unwrap()).unwrap();
}
#[test]
fn packages_are_deterministic_and_browser_permissions_are_separate() {
    let temp = tempfile::tempdir().unwrap();
    fixture(temp.path());
    let a = cors::build(temp.path()).unwrap();
    let b = cors::build(temp.path()).unwrap();
    assert_eq!(a, b);
    for item in a["packages"].as_array().unwrap() {
        let browser = item["browser"].as_str().unwrap();
        let name = item["file"].as_str().unwrap();
        let bytes = fs::read(temp.path().join("web/downloads").join(name)).unwrap();
        assert_eq!(hash(&bytes), item["sha256"]);
        assert_eq!(bytes.len() as u64, item["bytes"].as_u64().unwrap());
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut manifest = String::new();
        archive.by_name("manifest.json").unwrap().read_to_string(&mut manifest).unwrap();
        let manifest: serde_json::Value = serde_json::from_str(&manifest).unwrap();
        assert_eq!(manifest["permissions"].as_array().unwrap().iter().any(|p| p == "debugger"), browser == "chromium");
        assert_eq!(manifest["background"]["service_worker"].is_string(), browser == "chromium");
        if browser == "firefox" {
            assert_eq!(
                manifest["browser_specific_settings"]["gecko"]["data_collection_permissions"]["required"],
                serde_json::json!(["none"])
            );
        }
        for index in 0..archive.len() {
            let entry = archive.by_index(index).unwrap();
            assert_eq!(entry.last_modified().unwrap().year(), 2026);
            assert_eq!(entry.unix_mode().unwrap() & 0o777, 0o644);
            assert!(!entry.name().unwrap().contains(".."));
        }
        for size in [48, 128] {
            let mut png = vec![];
            archive.by_name(&format!("icon-{size}.png")).unwrap().read_to_end(&mut png).unwrap();
            assert_eq!(png, brand::png(size).unwrap());
        }
    }
}
fn decode_png(data: &[u8]) -> Vec<u8> {
    assert_eq!(&data[..8], b"\x89PNG\r\n\x1a\n");
    let mut position = 8;
    let mut compressed = vec![];
    while position < data.len() {
        let size = u32::from_be_bytes(data[position..position + 4].try_into().unwrap()) as usize;
        if &data[position + 4..position + 8] == b"IDAT" {
            compressed.extend(&data[position + 8..position + 8 + size]);
        }
        position += size + 12;
    }
    let mut result = vec![];
    flate2::read::ZlibDecoder::new(compressed.as_slice()).read_to_end(&mut result).unwrap();
    result
}
#[test]
fn downloads_are_whitelisted_and_checksum_verified() {
    let temp = tempfile::tempdir().unwrap();
    fixture(temp.path());
    cors::build(temp.path()).unwrap();
    fs::write(temp.path().join("web/downloads/private-canary.json"), "private").unwrap();
    let destination = temp.path().join("public");
    site::copy_downloads(temp.path(), &destination).unwrap();
    assert!(!destination.join("private-canary.json").exists());
    assert_eq!(fs::read_dir(destination).unwrap().count(), 4);
    let metadata = read_json(&temp.path().join("web/downloads/cors-helper.json")).unwrap();
    let package = metadata["packages"][0]["file"].as_str().unwrap();
    fs::write(temp.path().join("web/downloads").join(package), "tampered").unwrap();
    assert!(site::copy_downloads(temp.path(), &temp.path().join("bad")).is_err());
}
#[cfg(unix)]
#[test]
fn download_metadata_and_packages_cannot_be_symlinks() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    fixture(temp.path());
    cors::build(temp.path()).unwrap();
    let metadata = temp.path().join("web/downloads/cors-helper.json");
    let value = read_json(&metadata).unwrap();
    let copy = temp.path().join("metadata.json");
    fs::rename(&metadata, &copy).unwrap();
    symlink(&copy, &metadata).unwrap();
    assert!(site::copy_downloads(temp.path(), &temp.path().join("first")).is_err());
    fs::remove_file(&metadata).unwrap();
    fs::rename(copy, &metadata).unwrap();
    let package = temp.path().join("web/downloads").join(value["packages"][0]["file"].as_str().unwrap());
    let copy = temp.path().join("package.zip");
    fs::rename(&package, &copy).unwrap();
    symlink(copy, package).unwrap();
    assert!(site::copy_downloads(temp.path(), &temp.path().join("second")).is_err());
}
#[test]
fn safari_project_zip_refreshes_resources_and_excludes_user_state() {
    let temp = tempfile::tempdir().unwrap();
    fixture(temp.path());
    let manifest = read_json(&temp.path().join("extensions/cors-helper/manifest.json")).unwrap();
    let version = manifest["version"].as_str().unwrap();
    let project = temp.path().join(format!("dist/cors-helper/safari-project-{version}/Harness CORS Helper"));
    i_site_tool::write(&project.join("Harness CORS Helper Extension/Resources/stale.txt"), "stale").unwrap();
    i_site_tool::write(&project.join("Harness CORS Helper.xcodeproj/project.pbxproj"), "fixture").unwrap();
    i_site_tool::write(&project.join("Harness CORS Helper.xcodeproj/xcuserdata/private.txt"), "private").unwrap();
    i_site_tool::write(&project.join(".DS_Store"), "private").unwrap();
    let metadata = cors::safari(temp.path(), false).unwrap();
    assert_eq!(metadata["runtime_verified"], false);
    let bytes = fs::read(temp.path().join(metadata["file"].as_str().unwrap())).unwrap();
    assert_eq!(hash(&bytes), metadata["sha256"]);
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    assert!(zip.by_name("Harness CORS Helper/Harness CORS Helper.xcodeproj/project.pbxproj").is_ok());
    assert!(zip.by_name("Harness CORS Helper/Harness CORS Helper Extension/Resources/manifest.json").is_ok());
    for index in 0..zip.len() {
        let entry = zip.by_index(index).unwrap();
        let name = entry.name().unwrap();
        assert!(!name.contains("xcuserdata") && !name.contains(".DS_Store") && !name.contains("stale.txt"));
    }
}
#[cfg(unix)]
#[test]
fn safari_refuses_symlinked_resource_directory() {
    let temp = tempfile::tempdir().unwrap();
    fixture(temp.path());
    let manifest = read_json(&temp.path().join("extensions/cors-helper/manifest.json")).unwrap();
    let version = manifest["version"].as_str().unwrap();
    let parent = temp
        .path()
        .join(format!("dist/cors-helper/safari-project-{version}/Harness CORS Helper/Harness CORS Helper Extension"));
    fs::create_dir_all(&parent).unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), parent.join("Resources")).unwrap();
    assert!(cors::safari(temp.path(), false).unwrap_err().to_string().contains("symbolic link"));
}
#[test]
fn brand_icons_match_across_svg_pwa_and_extension_sizes() {
    let root = root();
    brand::sync_svg(&root, true).unwrap();
    let svg = brand::svg();
    assert!(svg.contains("viewBox=\"0 0 64 64\""));
    assert!(svg.contains("#22d3ee") && svg.contains("#0f172a"));
    assert_eq!(svg.matches("<circle ").count(), 1);
    assert_eq!(svg.matches("<rect x=\"29\"").count(), 5);
    assert_eq!(svg.matches("<rect x=").count(), 5);
    for size in [48, 128, 192, 512] {
        let bytes = brand::png(size).unwrap();
        let rows = decode_png(&bytes);
        for (x, y, expected) in [
            (0, 0, [15, 23, 42]),
            (32, 11, [34, 211, 238]),
            (32, 18, [15, 23, 42]),
            (32, 24, [34, 211, 238]),
            (23, 37, [15, 23, 42]),
            (32, 52, [34, 211, 238]),
            (25, 52, [15, 23, 42]),
            (39, 52, [15, 23, 42]),
        ] {
            let (x, y) = (x * size / 64, y * size / 64);
            let offset = (y * (size * 3 + 1) + 1 + x * 3) as usize;
            assert_eq!(&rows[offset..offset + 3], &expected, "{size}px at {x},{y}");
        }
        if size >= 192 {
            let x = size / 2;
            let y = 55 * size / 128;
            let offset = (y * (size * 3 + 1) + 1 + x * 3) as usize;
            assert_eq!(&rows[offset..offset + 3], &[15, 23, 42], "stem pixels must stay separate");
        }
    }
}
