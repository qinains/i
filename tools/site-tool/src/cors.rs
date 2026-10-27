use crate::{copy_tree, files, hash, read_json, text, write};
use anyhow::{Result, ensure};
use flate2::{Compression, write::ZlibEncoder};
use serde_json::{Value, json};
use std::{fs, io::Write, path::Path, process::Command};
const FILES: &[&str] = &[
    "rules.js",
    "actions.js",
    "endpoint.js",
    "background.js",
    "popup.html",
    "popup.css",
    "popup.js",
    "README.md",
    "LICENSE",
];
fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut result = (data.len() as u32).to_be_bytes().to_vec();
    result.extend(kind);
    result.extend(data);
    let mut crc = crc32fast::Hasher::new();
    crc.update(kind);
    crc.update(data);
    result.extend(crc.finalize().to_be_bytes());
    result
}
pub fn icon(size: u32) -> Result<Vec<u8>> {
    let mut rows = Vec::new();
    for y in 0..size {
        rows.push(0);
        for x in 0..size {
            let mark = f64::from(size) * 0.42 <= f64::from(x)
                && f64::from(x) < f64::from(size) * 0.58
                && ((f64::from(size) * 0.2 <= f64::from(y) && f64::from(y) < f64::from(size) * 0.35)
                    || (f64::from(size) * 0.45 <= f64::from(y) && f64::from(y) < f64::from(size) * 0.8));
            rows.extend(if mark { [80, 160, 255] } else { [24, 30, 40] });
        }
    }
    let mut compressed = ZlibEncoder::new(Vec::new(), Compression::best());
    compressed.write_all(&rows)?;
    let mut header = size.to_be_bytes().to_vec();
    header.extend(size.to_be_bytes());
    header.extend([8, 2, 0, 0, 0]);
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend(chunk(b"IHDR", &header));
    png.extend(chunk(b"IDAT", &compressed.finish()?));
    png.extend(chunk(b"IEND", &[]));
    Ok(png)
}
fn zip_files(source: &Path, names: &[String], destination: &Path) -> Result<()> {
    let mut archive = zip::ZipWriter::new(fs::File::create(destination)?);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(9))
        .last_modified_time(zip::DateTime::from_date_and_time(2026, 1, 1, 0, 0, 0).unwrap())
        .unix_permissions(0o644);
    let mut sorted = names.to_vec();
    sorted.sort();
    for name in sorted {
        let path = source.join(&name);
        ensure!(!fs::symlink_metadata(&path)?.file_type().is_symlink(), "source symlink: {name}");
        archive.start_file(name, options)?;
        archive.write_all(&fs::read(path)?)?;
    }
    archive.finish()?;
    Ok(())
}
pub fn build(root: &Path) -> Result<Value> {
    let source = root.join("extensions/cors-helper");
    let output = root.join("web/downloads");
    let stage = root.join("dist/cors-helper");
    fs::create_dir_all(&output)?;
    let manifest = read_json(&source.join("manifest.json"))?;
    let version = text(&manifest, "version")?;
    ensure!(regex::Regex::new(r"^\d+\.\d+\.\d+$")?.is_match(version), "invalid extension version");
    let mut packages = vec![];
    for browser in ["chromium", "firefox", "safari"] {
        let folder = stage.join(browser);
        fs::create_dir_all(&folder)?;
        let mut variant = manifest.clone();
        let mut names = FILES.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        if browser == "chromium" {
            let permissions =
                variant["permissions"].as_array_mut().ok_or_else(|| anyhow::anyhow!("permissions must be an array"))?;
            permissions.push("debugger".into());
            variant["minimum_chrome_version"] = "118".into();
            variant["background"] = json!({"service_worker":"background-debugger.js","type":"module"});
            names.extend(["preflight.js".into(), "background-debugger.js".into()]);
        } else {
            variant.as_object_mut().unwrap().remove("minimum_chrome_version");
            variant["background"] = json!({"scripts":["background.js"]});
        }
        if browser == "firefox" {
            variant["browser_specific_settings"] = json!({"gecko":{"id":"harness-cors-helper@lninl.local","strict_min_version":"142.0","data_collection_permissions":{"required":["none"]}}});
        }
        for name in &names {
            let path = if name == "LICENSE" { root.join("LICENSE") } else { source.join(name) };
            write(&folder.join(name), fs::read(path)?)?;
        }
        for size in [48, 128] {
            let name = format!("icon-{size}.png");
            write(&folder.join(&name), icon(size)?)?;
            names.push(name);
        }
        write(&folder.join("manifest.json"), serde_json::to_string_pretty(&variant)? + "\n")?;
        names.push("manifest.json".into());
        let name = format!("harness-cors-helper-{browser}-{version}.zip");
        let path = output.join(&name);
        zip_files(&folder, &names, &path)?;
        let bytes = fs::read(&path)?;
        packages.push(json!({"browser":browser,"file":name,"sha256":hash(&bytes),"bytes":bytes.len()}));
    }
    let metadata = json!({"version":version,"packages":packages});
    write(&output.join("cors-helper.json"), serde_json::to_string_pretty(&metadata)? + "\n")?;
    Ok(metadata)
}
pub fn safari(root: &Path, compile: bool) -> Result<Value> {
    build(root)?;
    let manifest = read_json(&root.join("extensions/cors-helper/manifest.json"))?;
    let version = text(&manifest, "version")?;
    let parent = root.join(format!("dist/cors-helper/safari-project-{version}"));
    if !parent.exists() {
        ensure!(
            Command::new("xcrun")
                .arg("safari-web-extension-converter")
                .arg(root.join("dist/cors-helper/safari"))
                .arg("--project-location")
                .arg(&parent)
                .args([
                    "--app-name",
                    "Harness CORS Helper",
                    "--bundle-identifier",
                    "local.lninl.Harness-CORS-Helper",
                    "--macos-only",
                    "--copy-resources",
                    "--no-open",
                    "--no-prompt"
                ])
                .status()?
                .success(),
            "Safari conversion failed"
        );
    }
    let folder = parent.join("Harness CORS Helper");
    let resources = folder.join("Harness CORS Helper Extension/Resources");
    if let Ok(meta) = fs::symlink_metadata(&resources) {
        ensure!(!meta.file_type().is_symlink(), "Generated Safari resources must not be a symbolic link");
        fs::remove_dir_all(&resources)?;
    }
    copy_tree(&root.join("dist/cors-helper/safari"), &resources)?;
    let output = root.join(format!("web/downloads/harness-cors-helper-safari-xcode-{version}.zip"));
    let names = files(&folder)?
        .iter()
        .filter(|path| {
            path.file_name().is_none_or(|name| name != ".DS_Store")
                && !path.components().any(|part| part.as_os_str() == "xcuserdata")
        })
        .map(|p| Ok(p.strip_prefix(&parent)?.to_string_lossy().replace('\\', "/")))
        .collect::<Result<Vec<_>>>()?;
    zip_files(&parent, &names, &output)?;
    if compile {
        ensure!(
            Command::new("xcodebuild")
                .arg("-project")
                .arg(folder.join("Harness CORS Helper.xcodeproj"))
                .args(["-scheme", "Harness CORS Helper", "-configuration", "Release", "-derivedDataPath"])
                .arg(root.join("dist/cors-helper/safari-build-final"))
                .args(["CODE_SIGNING_ALLOWED=NO", "build"])
                .status()?
                .success(),
            "Safari build failed"
        );
    }
    Ok(
        json!({"file":output.strip_prefix(root)?.to_string_lossy(),"sha256":hash(&fs::read(&output)?),"runtime_verified":false}),
    )
}
