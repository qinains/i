use i_wasm_package::inspect_package;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::io::{Cursor, Write};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};
fn wasm() -> Vec<u8> {
    wat::parse_str(
        r#"(module (memory (export "memory") 1 10)
    (func (export "harness_abi_version") (result i32) i32.const 1)
    (func (export "harness_alloc") (param i32) (result i32) i32.const 0)
    (func (export "harness_free") (param i32 i32))
    (func (export "harness_step") (param i32 i32) (result i64) i64.const 0))"#,
    )
    .unwrap()
}
fn package(extra: Option<&str>, method: CompressionMethod, bad_digest: bool) -> Vec<u8> {
    let wasm = wasm();
    let hash = format!("{:x}", Sha256::digest(&wasm));
    let manifest=json!({"format_version":1,"id":"example.count","name":"Count","version":"1.0.0","runtime":"harness-wasm-v1","entry":"plugin.wasm","sha256":if bad_digest {"0".repeat(64)}else{hash},"permissions":{"workspace":"read","storage":false},"tools":[{"name":"count","description":"Count","parameters":{"type":"object"}}]}).to_string();
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let opt = SimpleFileOptions::default().compression_method(method);
    for (name, bytes) in
        [("manifest.json", manifest.as_bytes()), ("plugin.wasm", wasm.as_slice()), ("LICENSE", b"MIT".as_slice())]
    {
        writer.start_file(name, opt).unwrap();
        writer.write_all(bytes).unwrap();
    }
    if let Some(name) = extra {
        writer.start_file(name, opt).unwrap();
        writer.write_all(b"extra").unwrap();
    }
    writer.finish().unwrap().into_inner()
}
#[test]
fn accepts_stored_and_deflated_packages() {
    for method in [CompressionMethod::Stored, CompressionMethod::Deflated] {
        assert!(inspect_package(&package(None, method, false)).is_ok());
    }
}
#[test]
fn rejects_extra_entries_path_traversal_digest_mismatch_and_bad_zip() {
    for name in ["../escape", "/absolute", "PLUGIN.WASM", "other.txt", "dir/file"] {
        assert!(inspect_package(&package(Some(name), CompressionMethod::Stored, false)).is_err());
    }
    assert!(inspect_package(&package(None, CompressionMethod::Stored, true)).is_err());
    assert!(inspect_package(b"not zip").is_err());
    assert!(inspect_package(&vec![0; 10 * 1024 * 1024 + 1]).is_err());
}
#[test]
fn rejects_crc_encryption_symlinks_and_central_directory_size_forgery() {
    let valid = package(None, CompressionMethod::Stored, false);
    let mut archive = zip::ZipArchive::new(Cursor::new(&valid)).unwrap();
    let license = archive.by_name("LICENSE").unwrap();
    let local = license.header_start() as usize;
    let central = license.central_header_start() as usize;
    let data = license.data_start() as usize;
    drop(license);
    drop(archive);
    let mut crc = valid.clone();
    crc[data] ^= 1;
    assert!(inspect_package(&crc).is_err());
    let mut encrypted = valid.clone();
    encrypted[local + 6] |= 1;
    encrypted[central + 8] |= 1;
    assert!(inspect_package(&encrypted).is_err());
    let mut symlink = valid.clone();
    symlink[central + 5] = 3;
    symlink[central + 38..central + 42].copy_from_slice(&(0o120777u32 << 16).to_le_bytes());
    assert!(inspect_package(&symlink).is_err());
    let mut malformed = valid;
    let end = malformed.len() - 22;
    malformed[end + 12..end + 16].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(inspect_package(&malformed).is_err());
}
#[test]
fn duplicate_names_are_rejected_before_zip_reader_deduplication() {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for name in ["manifest.json", "plugin.wasm", "PLUGIN.WASM"] {
        writer.start_file(name, SimpleFileOptions::default().compression_method(CompressionMethod::Stored)).unwrap();
        writer.write_all(b"x").unwrap();
    }
    let mut bytes = writer.finish().unwrap().into_inner();
    for offset in (0..bytes.len() - 11).filter(|&i| &bytes[i..i + 11] == b"PLUGIN.WASM").collect::<Vec<_>>() {
        bytes[offset..offset + 11].copy_from_slice(b"plugin.wasm");
    }
    assert!(inspect_package(&bytes).err().unwrap().to_string().contains("duplicate"));
}
#[test]
fn deflate_bomb_with_forged_uncompressed_size_is_rejected() {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in
        [("manifest.json", b"{}".to_vec()), ("plugin.wasm", wasm()), ("LICENSE", vec![b'A'; 256 * 1024 + 1])]
    {
        writer.start_file(name, SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)).unwrap();
        writer.write_all(&bytes).unwrap();
    }
    let mut bytes = writer.finish().unwrap().into_inner();
    let mut a = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
    let f = a.by_name("LICENSE").unwrap();
    let c = f.central_header_start() as usize;
    let l = f.header_start() as usize;
    drop(f);
    drop(a);
    bytes[c + 24..c + 28].copy_from_slice(&1u32.to_le_bytes());
    bytes[l + 22..l + 26].copy_from_slice(&1u32.to_le_bytes());
    assert!(inspect_package(&bytes).is_err());
}
#[test]
fn packer_writes_verified_package_and_refuses_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    let good = inspect_package(&package(None, CompressionMethod::Stored, false)).unwrap();
    std::fs::write(dir.path().join("plugin.wasm"), &good.wasm).unwrap();
    std::fs::write(dir.path().join("manifest.json"), serde_json::to_vec(&good.manifest).unwrap()).unwrap();
    std::fs::write(dir.path().join("LICENSE"), "MIT").unwrap();
    let out = dir.path().join("example.hplugin.zip");
    i_wasm_package::pack(dir.path(), &out).unwrap();
    let bytes = std::fs::read(&out).unwrap();
    assert!(inspect_package(&bytes).is_ok());
    assert!(i_wasm_package::pack(dir.path(), &out).is_err());
    assert_eq!(std::fs::read(&out).unwrap(), bytes);
}
