use anyhow::{Context, Result, ensure};
use serde_json::json;
use std::{collections::BTreeSet, fs, path::Path};
use walkdir::WalkDir;

const DIRECTORIES: &[&str] = &[
    ".github",
    "assets",
    "completions",
    "docs",
    "examples",
    "extensions",
    "locales",
    "plugins",
    "scripts",
    "server",
    "site",
    "src",
    "tests",
    "tools",
    "vendor",
    "web",
];
const FILES: &[&str] = &[
    ".gitattributes",
    ".gitignore",
    "Cargo.toml",
    "Cargo.lock",
    "rustfmt.toml",
    "LICENSE",
    "README.md",
    "README.zh-CN.md",
    "CONTRIBUTING.md",
    "SECURITY.md",
    "THIRD_PARTY_LICENSES.txt",
    ".gitleaks.toml",
];
const SKIP_PARTS: &[&str] = &[
    ".git",
    ".i",
    ".1k",
    ".gstack",
    ".superpowers",
    "superpowers",
    "target",
    "dist",
    "node_modules",
    "__pycache__",
    "test-results",
    "playwright-report",
    "xcuserdata",
];
const SKIP_PATHS: &[&str] = &[
    "examples/plugins.json",
    "docs/tools-composer-verification.md",
    "docs/dependency-size-history.md",
    "web/pkg",
    "web/plugin-pkg",
    "web/assets.json",
    "web/service-worker.js",
    "web/downloads",
    "web/icons/icon-192.png",
    "web/icons/icon-512.png",
    "docs/dependency-audit.md",
    "docs/dependency-size.md",
    "docs/size-report.md",
    "web/LICENSE.txt",
    "web/THIRD_PARTY_LICENSES.txt",
];

fn included(relative: &Path) -> bool {
    if relative.components().any(|part| SKIP_PARTS.iter().any(|skip| part.as_os_str() == *skip)) {
        return false;
    }
    if SKIP_PATHS.iter().any(|skip| relative.starts_with(skip)) {
        return false;
    }
    let name = relative.file_name().unwrap_or_default().to_string_lossy();
    name != ".DS_Store"
        && !name.starts_with(".env")
        && ![".pyc", ".local.json", ".pem", ".key", ".log", ".swp"].iter().any(|suffix| name.ends_with(suffix))
}
fn export(root: &Path, destination: &Path) -> Result<usize> {
    ensure!(
        fs::symlink_metadata(destination).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
        "Release snapshot already exists; choose a fresh snapshot rather than overwrite an audited tree"
    );
    let parent = destination.parent().context("snapshot parent directory")?;
    fs::create_dir_all(parent)?;
    let staging = tempfile::Builder::new().prefix("release-source-").tempdir_in(parent)?;
    let mut inventory = Vec::new();
    let names: BTreeSet<_> = DIRECTORIES.iter().chain(FILES).copied().collect();
    for name in names {
        let source = root.join(name);
        let metadata = match fs::symlink_metadata(&source) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        ensure!(!metadata.file_type().is_symlink(), "Release root must not be a symbolic link: {name}");
        let mut paths = Vec::new();
        if metadata.is_dir() {
            for entry in WalkDir::new(&source)
                .follow_links(false)
                .into_iter()
                .filter_entry(|entry| entry.path().strip_prefix(root).is_ok_and(included))
            {
                let entry = entry?;
                if entry.path() != source {
                    paths.push(entry.into_path());
                }
            }
        } else {
            paths.push(source);
        }
        paths.sort();
        for path in paths {
            let relative = path.strip_prefix(root)?;
            if !included(relative) {
                continue;
            }
            let metadata = fs::symlink_metadata(&path)?;
            ensure!(
                !metadata.file_type().is_symlink(),
                "Release source must not contain symbolic links: {}",
                relative.display()
            );
            if !metadata.is_file() {
                continue;
            }
            let target = staging.path().join(relative);
            fs::create_dir_all(target.parent().unwrap())?;
            fs::copy(&path, &target)?;
            // Preserve timestamps as copy2 did, in addition to fs::copy's permissions.
            let mut times = fs::FileTimes::new();
            if let Ok(value) = metadata.modified() {
                times = times.set_modified(value);
            }
            if let Ok(value) = metadata.accessed() {
                times = times.set_accessed(value);
            }
            fs::File::options().write(true).open(&target)?.set_times(times)?;
            inventory.push(json!({"path":relative.to_str().context("source path must be UTF-8")?.replace('\\',"/"),"sha256":crate::licenses::sha256(&fs::read(&target)?)}));
        }
    }
    ensure!(
        fs::symlink_metadata(destination).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
        "Snapshot destination appeared during export"
    );
    fs::rename(staging.path(), destination)?;
    fs::write(parent.join("release-inventory.json"), format!("{}\n", serde_json::to_string_pretty(&inventory)?))?;
    Ok(inventory.len())
}
pub(crate) fn run(root: &Path, args: &[String]) -> Result<()> {
    ensure!(args.is_empty(), "export-release accepts no arguments");
    let destination = root.join("dist/release-source");
    let count = export(root, &destination)?;
    println!("Exported {count} source files to {}; not yet audited or published", destination.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::Path};

    #[test]
    fn preserves_source_tests_licenses_scanner_and_rust_tools() {
        for name in [
            "LICENSE",
            "THIRD_PARTY_LICENSES.txt",
            "web/module-lexer.LICENSE",
            "tests/network.rs",
            "examples/wasm-word-count/src/lib.rs",
            ".github/workflows/pages.yml",
            ".gitleaks.toml",
            "tools/repo-tool/Cargo.lock",
        ] {
            assert!(included(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn excludes_private_and_generated_artifacts() {
        for name in [
            "examples/plugins.json",
            "docs/superpowers/plan.md",
            "web/node_modules/package/index.js",
            "web/pkg/i.wasm",
            "examples/wasm-word-count/target/file",
            "web/downloads/private.json",
            "server/.env",
            "plugins/key.local.json",
            "site/__pycache__/x.pyc",
            ".DS_Store",
            "tools/repo-tool/target/debug/i-repo-tool",
        ] {
            assert!(!included(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn source_export_is_no_clobber_and_inventory_matches_copied_bytes() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("LICENSE"), "fixture license\n").unwrap();
        fs::create_dir(root.path().join("tools")).unwrap();
        fs::write(root.path().join("tools/source.rs"), "fn main() {}\n").unwrap();
        let destination = root.path().join("dist/release-source");
        let count = export(root.path(), &destination).unwrap();
        assert_eq!(count, 2);
        let inventory: serde_json::Value =
            serde_json::from_slice(&fs::read(root.path().join("dist/release-inventory.json")).unwrap()).unwrap();
        assert_eq!(inventory[0]["path"], "LICENSE");
        assert_eq!(inventory[0]["sha256"], crate::licenses::sha256(b"fixture license\n"));
        assert!(export(root.path(), &destination).is_err());
        assert_eq!(fs::read_to_string(destination.join("LICENSE")).unwrap(), "fixture license\n");
    }

    #[test]
    #[cfg(unix)]
    fn rejects_top_level_and_nested_symlinks_without_publishing_snapshot() {
        for top in [true, false] {
            let root = tempfile::tempdir().unwrap();
            let external = tempfile::tempdir().unwrap();
            fs::write(external.path().join("secret"), "not source").unwrap();
            if top {
                std::os::unix::fs::symlink(external.path(), root.path().join("assets")).unwrap();
            } else {
                fs::create_dir(root.path().join("assets")).unwrap();
                std::os::unix::fs::symlink(external.path().join("secret"), root.path().join("assets/link")).unwrap();
            }
            let dest = root.path().join("dist/release-source");
            assert!(export(root.path(), &dest).is_err());
            assert!(!dest.exists());
        }
    }
}
