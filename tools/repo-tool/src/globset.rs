use anyhow::{Context, Result, ensure};
use std::{fs, path::Path};
use walkdir::WalkDir;

fn validate(root: &toml::Value, runtime: &toml::Value, lock: &toml::Value, local_override: bool) -> Result<()> {
    let dependency = root.get("dependencies").and_then(|v| v.get("globset"));
    let expected: toml::Value = toml::from_str("version='=0.4.20'\ndefault-features=false\n")?;
    ensure!(
        dependency == Some(&expected)
            && runtime.get("dependencies").and_then(|v| v.get("globset")).and_then(toml::Value::as_str)
                == Some("=0.4.20"),
        "Review and update the official globset pin explicitly"
    );
    ensure!(
        !local_override && root.get("patch").and_then(|v| v.get("crates-io")).and_then(|v| v.get("globset")).is_none(),
        "A local globset override is not allowed"
    );
    let entries: Vec<_> = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .context("lock packages")?
        .iter()
        .filter(|p| p.get("name").and_then(toml::Value::as_str) == Some("globset"))
        .collect();
    ensure!(entries.len() == 1, "globset must have one checksum-locked crates.io source");
    let entry = entries[0];
    let checksum = entry.get("checksum").and_then(toml::Value::as_str).unwrap_or("");
    ensure!(
        entry.get("version").and_then(toml::Value::as_str) == Some("0.4.20")
            && entry.get("source").and_then(toml::Value::as_str)
                == Some("registry+https://github.com/rust-lang/crates.io-index")
            && checksum.len() == 64
            && checksum.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "globset must have one checksum-locked crates.io source"
    );
    Ok(())
}
pub(crate) fn run(root: &Path) -> Result<()> {
    let vendor = root.join("vendor/globset");
    let mut local = false;
    if fs::symlink_metadata(&vendor).is_ok() {
        for entry in WalkDir::new(vendor).follow_links(false) {
            let entry = entry?;
            local |= entry.file_type().is_file() || entry.file_type().is_symlink();
        }
    }
    validate(
        &toml::from_str(&fs::read_to_string(root.join("Cargo.toml"))?)?,
        &toml::from_str(&fs::read_to_string(root.join("plugins/runtime/Cargo.toml"))?)?,
        &toml::from_str(&fs::read_to_string(root.join("Cargo.lock"))?)?,
        local,
    )?;
    println!("PASS: official globset 0.4.20, locked registry checksum, no local patch");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globset_requires_official_pin_and_unique_checksum_locked_source() {
        let root: toml::Value =
            toml::from_str("[dependencies.globset]\nversion='=0.4.20'\ndefault-features=false\n").unwrap();
        let runtime: toml::Value = toml::from_str("[dependencies]\nglobset='=0.4.20'\n").unwrap();
        let lock: toml::Value = toml::from_str(&format!("[[package]]\nname='globset'\nversion='0.4.20'\nsource='registry+https://github.com/rust-lang/crates.io-index'\nchecksum='{}'\n", "a".repeat(64))).unwrap();
        assert!(validate(&root, &runtime, &lock, false).is_ok());
        assert!(validate(&root, &runtime, &lock, true).is_err());
        let mut bad = root.clone();
        bad["dependencies"]["globset"]["version"] = toml::Value::String("0.4".into());
        assert!(validate(&bad, &runtime, &lock, false).is_err());
        let mut bad = lock.clone();
        bad["package"][0]["checksum"] = toml::Value::String("not-a-checksum".into());
        assert!(validate(&root, &runtime, &bad, false).is_err());
    }
}
