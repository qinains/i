use super::*;
use std::{io::Write, path::Path};
pub fn pack(dir: &Path, out: &Path) -> Result<()> {
    ensure!(!out.exists(), "output already exists");
    let wasm = std::fs::read(dir.join("plugin.wasm"))?;
    inspect_module(&wasm)?;
    let mut manifest: Manifest = serde_json::from_slice(&std::fs::read(dir.join("manifest.json"))?)?;
    manifest.sha256 = format!("{:x}", Sha256::digest(&wasm));
    manifest.validate()?;
    let license = std::fs::read(dir.join("LICENSE"))?;
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opt = zip::write::SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(zip::DateTime::default());
    for (name, bytes) in
        [("manifest.json", serde_json::to_vec_pretty(&manifest)?), ("plugin.wasm", wasm), ("LICENSE", license)]
    {
        writer.start_file(name, opt)?;
        writer.write_all(&bytes)?;
    }
    let bytes = writer.finish()?.into_inner();
    inspect_package(&bytes)?;
    // create_new avoids silently overwriting an input or another process's output.
    let parent = out.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let temp = parent.join(format!(".i-plugin-pack-{}.tmp", std::process::id()));
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&temp)?;
    let result = (|| -> Result<()> {
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::hard_link(&temp, out)?;
        Ok(())
    })();
    let cleanup = std::fs::remove_file(&temp);
    result?;
    cleanup?;
    Ok(())
}
