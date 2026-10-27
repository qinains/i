mod module;
#[cfg(not(target_arch = "wasm32"))]
mod pack;
#[cfg(all(target_arch = "wasm32", feature = "web"))]
mod web;
use anyhow::{Context, Result, ensure};
use i_wasm_protocol::Manifest;
pub use module::inspect_module;
#[cfg(not(target_arch = "wasm32"))]
pub use pack::pack;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Cursor, Read},
};
use zip::{CompressionMethod, ZipArchive};

pub const PACKAGE_LIMIT: usize = 10 * 1024 * 1024;
pub struct VerifiedPackage {
    pub manifest: Manifest,
    pub wasm: Vec<u8>,
    pub license: String,
    pub digest: String,
}
fn u16_at(bytes: &[u8], at: usize) -> Result<usize> {
    Ok(u16::from_le_bytes(bytes.get(at..at + 2).context("truncated ZIP")?.try_into()?) as usize)
}
fn u32_at(bytes: &[u8], at: usize) -> Result<usize> {
    Ok(u32::from_le_bytes(bytes.get(at..at + 4).context("truncated ZIP")?.try_into()?) as usize)
}
// Keep the container subset explicit, before ZipArchive allocates its directory.
fn check_directory(bytes: &[u8]) -> Result<()> {
    ensure!(bytes.len() >= 22 && bytes.len() <= PACKAGE_LIMIT, "plugin package exceeds 10 MiB or is truncated");
    let end = bytes.len() - 22;
    ensure!(bytes.get(end..end + 4) == Some(b"PK\x05\x06".as_slice()), "ZIP comments and ZIP64 are not supported");
    ensure!(
        u16_at(bytes, end + 4)? == 0
            && u16_at(bytes, end + 6)? == 0
            && u16_at(bytes, end + 8)? == 3
            && u16_at(bytes, end + 10)? == 3
            && u16_at(bytes, end + 20)? == 0,
        "ZIP must contain exactly three files on one disk"
    );
    let (start, size) = (u32_at(bytes, end + 16)?, u32_at(bytes, end + 12)?);
    ensure!(start.checked_add(size) == Some(end), "invalid ZIP directory extent");
    let mut pos = start;
    let mut names = std::collections::BTreeSet::new();
    for _ in 0..3 {
        ensure!(bytes.get(pos..pos + 4) == Some(b"PK\x01\x02".as_slice()), "invalid ZIP central entry");
        let (n, extra, comment) = (u16_at(bytes, pos + 28)?, u16_at(bytes, pos + 30)?, u16_at(bytes, pos + 32)?);
        let next = pos.checked_add(46 + n + extra + comment).context("ZIP directory overflow")?;
        ensure!(next <= end, "truncated ZIP directory");
        let name = std::str::from_utf8(bytes.get(pos + 46..pos + 46 + n).context("truncated ZIP name")?)?;
        ensure!(
            ["manifest.json", "plugin.wasm", "LICENSE"].contains(&name) && names.insert(name),
            "unexpected or duplicate ZIP entry"
        );
        let local = u32_at(bytes, pos + 42)?;
        ensure!(
            local < start && bytes.get(local..local + 4) == Some(b"PK\x03\x04".as_slice()),
            "invalid ZIP local header"
        );
        let local_n = u16_at(bytes, local + 26)?;
        ensure!(bytes.get(local + 30..local + 30 + local_n) == Some(name.as_bytes()), "ZIP local/central names differ");
        pos = next;
    }
    ensure!(pos == end, "unexpected ZIP directory data");
    Ok(())
}
pub fn inspect_package(bytes: &[u8]) -> Result<VerifiedPackage> {
    check_directory(bytes)?;
    let mut archive = ZipArchive::new(Cursor::new(bytes))?;
    ensure!(archive.len() == 3 && archive.offset() == 0, "invalid ZIP file directory");
    let mut entries = BTreeMap::new();
    let mut total = 0;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        let name = file.name()?.into_owned();
        ensure!(
            !file.encrypted()
                && !file.is_symlink()
                && file.is_file()
                && matches!(file.compression(), CompressionMethod::Stored | CompressionMethod::Deflated),
            "unsupported ZIP entry type/encryption/compression"
        );
        let limit = match name.as_str() {
            "manifest.json" => 64 * 1024,
            "plugin.wasm" => 8 * 1024 * 1024,
            "LICENSE" => 256 * 1024,
            _ => anyhow::bail!("unexpected ZIP file"),
        };
        ensure!(file.size() <= limit as u64, "plugin file exceeds byte limit");
        let mut content = Vec::new();
        file.by_ref().take(limit as u64 + 1).read_to_end(&mut content)?;
        total += content.len();
        ensure!(content.len() <= limit && total <= PACKAGE_LIMIT, "decompressed plugin exceeds byte limit");
        ensure!(entries.insert(name, content).is_none(), "duplicate ZIP file");
    }
    let manifest: Manifest = serde_json::from_slice(entries.get("manifest.json").context("missing manifest")?)?;
    manifest.validate()?;
    let wasm = entries.remove("plugin.wasm").context("missing WASM")?;
    let digest = format!("{:x}", Sha256::digest(&wasm));
    ensure!(digest == manifest.sha256, "plugin WASM digest mismatch");
    inspect_module(&wasm)?;
    let license = String::from_utf8(entries.remove("LICENSE").context("missing LICENSE")?)?;
    ensure!(!license.trim().is_empty(), "empty LICENSE");
    Ok(VerifiedPackage { manifest, wasm, license, digest })
}
