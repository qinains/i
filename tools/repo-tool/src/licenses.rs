use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use walkdir::WalkDir;

const LIMIT: u64 = 1024 * 1024;
const FIRST_PARTY: &[&str] = &[
    "i",
    "i-plugins",
    "i-relay",
    "i-wasm-protocol",
    "i-wasm-package",
    "i-wasm-sdk",
    "i-chatgpt",
    "i-repo-tool",
    "i-native-tool",
    "i-site-tool",
    "i-test-fixtures",
];
const PUBLIC_REPOSITORIES: &[&str] =
    &["madsmtm/objc2", "DoumanAsh/clipboard-win", "r-efi/r-efi", "bytecodealliance/wasm-tools", "rusticata/asn1-rs"];

pub(crate) fn sha256(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key].as_str().with_context(|| format!("missing {key}"))
}
fn key(package: &Value) -> Result<String> {
    Ok(format!("{}@{}", text(package, "name")?, text(package, "version")?))
}
fn first_party(package: &Value, root: &Path) -> bool {
    FIRST_PARTY.contains(&package["name"].as_str().unwrap_or(""))
        && package["source"].is_null()
        && package["manifest_path"]
            .as_str()
            .and_then(|p| Path::new(p).canonicalize().ok())
            .is_some_and(|p| p.starts_with(root))
}
fn read_index(directory: &Path) -> Result<Value> {
    let path = directory.join("index.json");
    Ok(if path.try_exists()? { serde_json::from_slice(&fs::read(path)?)? } else { json!({}) })
}
fn checked_file(path: &Path, root: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        !metadata.file_type().is_symlink()
            && metadata.is_file()
            && metadata.len() <= LIMIT
            && path.canonicalize()?.starts_with(root.canonicalize()?),
        "Invalid upstream license file: {}",
        path.display()
    );
    Ok(())
}
fn hex(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn verify_blob(data: &[u8], identity: &str) -> Result<()> {
    ensure!(data.len() <= LIMIT as usize, "Upstream notice exceeds size limit");
    let mut hasher = sha1::Sha1::new();
    hasher.update(format!("blob {}\0", data.len()).as_bytes());
    hasher.update(data);
    ensure!(
        hex(identity, 40) && format!("{:x}", hasher.finalize()) == identity,
        "Upstream license blob verification failed"
    );
    Ok(())
}
fn supplement_files(package: &Value, directory: &Path) -> Result<Vec<PathBuf>> {
    let index = read_index(directory)?;
    let mut files = Vec::new();
    if let Some(entries) = index[&key(package)?].get("files") {
        for entry in entries.as_array().context("supplement files must be an array")? {
            let file = text(entry, "file")?;
            ensure!(file.strip_suffix(".txt").is_some_and(|stem| hex(stem, 64)), "Invalid license supplement filename");
            let path = directory.join(file);
            checked_file(&path, directory)?;
            ensure!(sha256(&fs::read(&path)?) == text(entry, "sha256")?, "License supplement checksum mismatch");
            files.push(path);
        }
    }
    Ok(files)
}
fn prefix(name: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| name.to_ascii_uppercase().starts_with(prefix))
}
fn license_files(package: &Value, supplements: &Path) -> Result<Vec<PathBuf>> {
    let root = Path::new(text(package, "manifest_path")?).parent().context("manifest directory")?.canonicalize()?;
    let mut candidates = BTreeSet::new();
    for entry in fs::read_dir(&root)? {
        let path = entry?.path();
        if !prefix(
            &path.file_name().unwrap().to_string_lossy(),
            &["LICENSE", "LICENCE", "COPYING", "COPYRIGHT", "NOTICE", "AUTHORS"],
        ) {
            continue;
        }
        ensure!(!fs::symlink_metadata(&path)?.file_type().is_symlink(), "Invalid upstream license symlink");
        if path.is_dir() {
            for entry in WalkDir::new(&path).follow_links(false) {
                let entry = entry?;
                ensure!(!entry.file_type().is_symlink(), "Invalid upstream license symlink");
                if entry.file_type().is_file() {
                    candidates.insert(entry.into_path());
                }
            }
        } else {
            candidates.insert(path);
        }
    }
    let declared = package.get("license_file").and_then(Value::as_str);
    if let Some(path) = declared {
        candidates.insert(root.join(path));
    }
    let mut inline_mit = false;
    for path in &candidates {
        checked_file(path, &root)?;
        if prefix(&path.file_name().unwrap().to_string_lossy(), &["AUTHORS"]) {
            let value = fs::read_to_string(path)?;
            inline_mit |= ["Permission is hereby granted", "THE SOFTWARE IS PROVIDED", "COPYRIGHT:"]
                .iter()
                .all(|marker| value.contains(marker));
        }
    }
    let primary = declared.is_some()
        || candidates
            .iter()
            .any(|p| prefix(&p.file_name().unwrap().to_string_lossy(), &["LICENSE", "LICENCE", "COPYING"]));
    if !primary && !inline_mit {
        let files = supplement_files(package, supplements)?;
        ensure!(!files.is_empty(), "Missing upstream license text for {}", key(package)?);
        candidates.extend(files);
    }
    Ok(candidates.into_iter().collect())
}
fn identity(package: &Value) -> (String, String, Option<String>) {
    (
        package["name"].as_str().unwrap_or("").into(),
        package["version"].as_str().unwrap_or("").into(),
        package["source"].as_str().map(str::to_owned),
    )
}
fn check_lock_coverage(metadata: &Value, lock: &toml::Value) -> Result<()> {
    let actual: BTreeSet<_> =
        metadata["packages"].as_array().context("metadata packages")?.iter().map(identity).collect();
    let mut missing = Vec::new();
    for entry in lock["package"].as_array().context("lock packages")? {
        let package = serde_json::to_value(entry)?;
        let id = identity(&package);
        if !actual.contains(&id) {
            missing.push(format!("{}@{}", id.0, id.1));
        }
    }
    ensure!(missing.is_empty(), "License metadata omitted locked packages: {}", missing.join(", "));
    Ok(())
}
fn render(packages: &[Value], root: &Path, supplements: &Path) -> Result<String> {
    let mut packages = packages.to_vec();
    packages
        .sort_by_key(|p| (p["name"].as_str().unwrap_or("").to_owned(), p["version"].as_str().unwrap_or("").to_owned()));
    let mut texts = BTreeMap::new();
    let mut records = Vec::new();
    let mut missing = Vec::new();
    for package in packages {
        if first_party(&package, root) {
            continue;
        }
        let files = match license_files(&package, supplements) {
            Ok(files) => files,
            Err(error) => {
                missing.push(format!("{}: {error:#}", key(&package)?));
                continue;
            }
        };
        let mut references = Vec::new();
        for file in files {
            let value = fs::read_to_string(&file)?.replace("\r\n", "\n").trim().to_owned();
            let digest = sha256(value.as_bytes());
            references.push(format!("{} [{}]", file.file_name().unwrap().to_string_lossy(), digest));
            texts.insert(digest, value);
        }
        records.push(format!(
            "{} {}\nDeclared license: {}\nNotices: {}",
            text(&package, "name")?,
            text(&package, "version")?,
            package["license"].as_str().unwrap_or("see upstream text"),
            references.join("; ")
        ));
    }
    ensure!(missing.is_empty(), "Missing upstream license texts: {}", missing.join(", "));
    Ok(format!(
        "Third-party Rust dependency licenses and notices\n\nFirst-party i / lninl code is MIT licensed. Dependencies retain their own licenses.\nThis is a conservative superset of locked build, test and target dependencies,\nnot a claim that every listed crate is linked into every binary.\nVendored JavaScript notices are separately retained in web/module-lexer.LICENSE\nand web/module-lexer.NOTICE.\n\n{}\n\n{}\n",
        records.join("\n\n"),
        texts.into_iter().map(|(key, value)| format!("===== {key} =====\n{value}")).collect::<Vec<_>>().join("\n\n")
    ))
}
fn github(endpoint: &str, cache: &mut BTreeMap<String, Value>) -> Result<Value> {
    if let Some(value) = cache.get(endpoint) {
        return Ok(value.clone());
    }
    let mut response = ureq::get(&format!("https://api.github.com/{endpoint}"))
        .header("User-Agent", "i-repo-tool-license-audit")
        .call()?;
    let data = response.body_mut().with_config().limit(4 * LIMIT).read_to_vec()?;
    let value: Value = serde_json::from_slice(&data)?;
    cache.insert(endpoint.into(), value.clone());
    Ok(value)
}
fn fetch_missing(packages: &[Value], root: &Path, directory: &Path) -> Result<()> {
    fs::create_dir_all(directory)?;
    let mut records = read_index(directory)?;
    let mut cache = BTreeMap::new();
    for package in packages {
        if first_party(package, root) || license_files(package, directory).is_ok() {
            continue;
        }
        let mut files = Vec::new();
        let mut record;
        if matches!(package["name"].as_str(), Some("winapi-i686-pc-windows-gnu" | "winapi-x86_64-pc-windows-gnu")) {
            let upstream = packages.iter().find(|p| p["name"] == "winapi").context("winapi parent missing")?;
            for path in license_files(upstream, directory)? {
                files.push((path.file_name().unwrap().to_string_lossy().into_owned(), fs::read(path)?));
            }
            record = json!({"source":format!("https://crates.io/crates/winapi/{}",text(upstream,"version")?),"note":"Architecture import-library crates share the parent winapi-rs repository notices."});
        } else {
            let repository = text(package, "repository")?
                .strip_prefix("https://github.com/")
                .context("Review upstream repository before fetching")?
                .trim_end_matches('/')
                .trim_end_matches(".git");
            ensure!(
                PUBLIC_REPOSITORIES.contains(&repository),
                "Review the upstream repository before fetching licenses for {}",
                key(package)?
            );
            let vcs: Value = serde_json::from_slice(&fs::read(
                Path::new(text(package, "manifest_path")?).with_file_name(".cargo_vcs_info.json"),
            )?)?;
            let revision = text(&vcs["git"], "sha1")?;
            ensure!(hex(revision, 40), "Invalid pinned upstream revision");
            let tree = github(&format!("repos/{repository}/git/trees/{revision}"), &mut cache)?;
            for entry in tree["tree"].as_array().context("upstream tree")? {
                let name = text(entry, "path")?;
                if entry["type"] != "blob" || !prefix(name, &["LICENSE", "LICENCE", "COPYING", "NOTICE"]) {
                    continue;
                }
                let blob = text(entry, "sha")?;
                ensure!(hex(blob, 40), "Invalid upstream blob identity");
                let value = github(&format!("repos/{repository}/git/blobs/{blob}"), &mut cache)?;
                ensure!(value["encoding"] == "base64", "Unsupported upstream blob encoding");
                let data = STANDARD.decode(text(&value, "content")?.split_whitespace().collect::<String>())?;
                verify_blob(&data, blob)?;
                files.push((name.to_owned(), data));
            }
            ensure!(!files.is_empty(), "No root notices at pinned upstream revision for {}", key(package)?);
            record = json!({"repository":format!("https://github.com/{repository}"),"revision":revision});
        }
        let mut entries = Vec::new();
        for (name, data) in files {
            let digest = sha256(&data);
            let filename = format!("{digest}.txt");
            fs::write(directory.join(&filename), data)?;
            entries.push(json!({"upstream_file":name,"file":filename,"sha256":digest}));
        }
        record["files"] = json!(entries);
        records[key(package)?] = record;
        fs::write(directory.join("index.json"), format!("{}\n", serde_json::to_string_pretty(&records)?))?;
    }
    Ok(())
}
fn metadata(root: &Path, manifest: Option<&Path>) -> Result<Value> {
    let mut command = Command::new("cargo");
    command.args(["metadata", "--all-features", "--locked", "--offline", "--format-version", "1"]).current_dir(root);
    if let Some(path) = manifest {
        command.arg("--manifest-path").arg(path);
    }
    let output = command.output()?;
    ensure!(output.status.success(), "cargo metadata failed: {}", String::from_utf8_lossy(&output.stderr));
    let metadata: Value = serde_json::from_slice(&output.stdout)?;
    let lock: toml::Value =
        toml::from_str(&fs::read_to_string(Path::new(text(&metadata, "workspace_root")?).join("Cargo.lock"))?)?;
    check_lock_coverage(&metadata, &lock)?;
    Ok(metadata)
}
fn tool_manifests(root: &Path) -> Result<Vec<PathBuf>> {
    let directory = root.join("tools");
    if !directory.try_exists()? {
        return Ok(Vec::new());
    }
    let mut manifests = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let manifest = entry.path().join("Cargo.toml");
        if entry.file_type()?.is_dir() && manifest.is_file() {
            manifests.push(manifest);
        }
    }
    manifests.sort();
    Ok(manifests)
}
pub(crate) fn run(root: &Path, args: &[String]) -> Result<()> {
    let (mut check, mut fetch, mut manifest, mut include, mut output) = (false, false, None, None, None);
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let (flag, inline) = arg.split_once('=').map_or((arg.as_str(), None), |(key, value)| (key, Some(value)));
        match flag {
            "--check" => check = true,
            "--fetch-missing" => fetch = true,
            "--manifest-path" | "--include-license" | "--output" => {
                let value = inline
                    .or_else(|| iter.next().map(String::as_str))
                    .with_context(|| format!("{flag} requires a path"))?;
                ensure!(!value.is_empty(), "{flag} requires a path");
                let path = PathBuf::from(value);
                match flag {
                    "--manifest-path" => manifest = Some(path.canonicalize()?),
                    "--include-license" => include = Some(path),
                    _ => output = Some(path),
                }
            }
            _ => bail!("unknown licenses option: {arg}"),
        }
        ensure!(inline.is_none() || !matches!(flag, "--check" | "--fetch-missing"), "{flag} does not accept a value");
    }
    ensure!(
        manifest.is_none() || output.is_some(),
        "--manifest-path requires an explicit --output; the main notices must not be replaced by a plugin subset"
    );
    ensure!(include.is_none() || manifest.is_some(), "--include-license requires --manifest-path");
    let main = metadata(root, manifest.as_deref())?;
    let mut packages = main["packages"].as_array().context("metadata packages")?.clone();
    // Independent repository tools have their own locks; their dependencies must not disappear.
    if manifest.is_none() {
        for tool in tool_manifests(root)? {
            for package in metadata(root, Some(&tool))?["packages"].as_array().context("tool packages")? {
                if !packages.iter().any(|old| identity(old) == identity(package)) {
                    packages.push(package.clone());
                }
            }
        }
    }
    let count = packages.len();
    let supplements = root.join("vendor/licenses");
    if fetch {
        fetch_missing(&packages, root, &supplements)?;
    }
    if let Some(manifest) = &manifest {
        packages.retain(|p| {
            p["manifest_path"].as_str().and_then(|p| Path::new(p).canonicalize().ok()).as_ref() != Some(manifest)
        });
    }
    let mut rendered = render(&packages, root, &supplements)?;
    if let Some(path) = include {
        rendered = format!("{}\n\n{rendered}", fs::read_to_string(path)?);
    }
    if manifest.is_some() && packages.iter().any(|p| p["name"] == "i-wasm-sdk") {
        rendered.push_str(&format!("\nHarness WASM SDK license:\n\n{}", fs::read_to_string(root.join("LICENSE"))?));
    }
    let destination = output.unwrap_or_else(|| root.join("THIRD_PARTY_LICENSES.txt"));
    if check {
        ensure!(
            fs::read_to_string(&destination).ok().as_deref() == Some(&rendered),
            "Third-party notices are stale; run cargo run --manifest-path tools/repo-tool/Cargo.toml -- licenses"
        );
    } else {
        fs::write(destination, &rendered)?;
    }
    println!("Verified {count} locked packages; notices: {} bytes", rendered.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(root: &Path, name: &str) -> Value {
        json!({"name":name,"version":"1.0.0","license":"MIT","manifest_path":root.join("Cargo.toml")})
    }

    #[test]
    fn finds_all_independent_repository_tool_workspaces() {
        let dir = tempfile::tempdir().unwrap();
        assert!(tool_manifests(dir.path()).unwrap().is_empty());
        for name in ["a", "b"] {
            fs::create_dir_all(dir.path().join("tools").join(name)).unwrap();
            fs::write(dir.path().join("tools").join(name).join("Cargo.toml"), "[package]").unwrap();
        }
        assert_eq!(
            tool_manifests(dir.path()).unwrap(),
            vec![dir.path().join("tools/a/Cargo.toml"), dir.path().join("tools/b/Cargo.toml")]
        );
    }

    #[test]
    fn pinned_git_blobs_are_verified_before_storing_supplements() {
        assert!(verify_blob(b"test content\n", "d670460b4b4aece5915caf5c68d12f560a9fe3e4").is_ok());
        assert!(verify_blob(b"tampered", "d670460b4b4aece5915caf5c68d12f560a9fe3e4").is_err());
        assert!(verify_blob(&vec![b'x'; LIMIT as usize + 1], &"a".repeat(40)).is_err());
    }

    #[test]
    fn hashed_vendor_notices_are_exempt_from_git_line_ending_conversion() {
        let attributes = fs::read_to_string(crate::root().join(".gitattributes")).unwrap();
        assert!(attributes.lines().any(|line| line.trim() == "vendor/licenses/*.txt -text"));
        assert!(attributes.lines().any(|line| line.trim() == "THIRD_PARTY_LICENSES.txt -text"));
    }

    #[test]
    fn authors_with_complete_inline_mit_terms_are_retained_without_supplement() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("AUTHORS"),
            "COPYRIGHT: fixture\nPermission is hereby granted\nTHE SOFTWARE IS PROVIDED",
        )
        .unwrap();
        assert_eq!(license_files(&package(dir.path(), "fixture"), &dir.path().join("absent")).unwrap().len(), 1);
    }

    #[test]
    fn declared_license_cannot_escape_crate_and_oversized_notices_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        let mut pkg = package(dir.path(), "fixture");
        pkg["license_file"] = json!(outside.path());
        assert!(license_files(&pkg, &dir.path().join("absent")).is_err());
        fs::write(dir.path().join("LICENSE"), vec![b'x'; LIMIT as usize + 1]).unwrap();
        assert!(license_files(&package(dir.path(), "fixture"), &dir.path().join("absent")).is_err());
    }

    #[test]
    fn preserves_case_insensitive_notices_and_authors() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("license-mit"), "copyright and permission").unwrap();
        fs::write(dir.path().join("AUTHORS"), "upstream attribution").unwrap();
        let paths = license_files(&package(dir.path(), "fixture"), &dir.path().join("absent")).unwrap();
        assert_eq!(
            paths.iter().map(|p| p.file_name().unwrap().to_str().unwrap()).collect::<Vec<_>>(),
            ["AUTHORS", "license-mit"]
        );
    }

    #[test]
    fn missing_license_and_names_only_authors_are_not_silently_accepted() {
        let dir = tempfile::tempdir().unwrap();
        assert!(license_files(&package(dir.path(), "fixture"), &dir.path().join("absent")).is_err());
        fs::write(dir.path().join("AUTHORS"), "Names only").unwrap();
        assert!(license_files(&package(dir.path(), "fixture"), &dir.path().join("absent")).is_err());
    }

    #[test]
    fn complete_lock_coverage_includes_optional_target_and_tooling_packages() {
        let locked =
            toml::from_str::<toml::Value>("[[package]]\nname='fixture'\nversion='1.0.0'\nsource='registry+fixture'\n")
                .unwrap();
        assert!(check_lock_coverage(&json!({"packages":[]}), &locked).is_err());
        assert!(
            check_lock_coverage(
                &json!({"packages":[{"name":"fixture","version":"1.0.0","source":"registry+fixture"}]}),
                &locked
            )
            .is_ok()
        );
        assert!(
            check_lock_coverage(
                &json!({"packages":[{"name":"fixture","version":"1.0.0","source":"registry+other"}]}),
                &locked
            )
            .is_err()
        );
    }

    #[test]
    fn supplement_checksum_and_filename_are_verified() {
        let dir = tempfile::tempdir().unwrap();
        let data = b"copyright and permission";
        let hash = sha256(data);
        let target = dir.path().join(format!("{hash}.txt"));
        fs::write(&target, data).unwrap();
        fs::write(
            dir.path().join("index.json"),
            json!({"fixture@1.0.0":{"files":[{"file":format!("{hash}.txt"),"sha256":hash}]}}).to_string(),
        )
        .unwrap();
        let pkg = package(dir.path(), "fixture");
        assert_eq!(supplement_files(&pkg, dir.path()).unwrap(), vec![target.clone()]);
        fs::write(target, "changed").unwrap();
        assert!(supplement_files(&pkg, dir.path()).is_err());
        fs::write(
            dir.path().join("index.json"),
            json!({"fixture@1.0.0":{"files":[{"file":"../private","sha256":"0".repeat(64)}]}}).to_string(),
        )
        .unwrap();
        assert!(supplement_files(&pkg, dir.path()).is_err());
    }

    #[test]
    fn rendering_is_stable_and_deduplicates_normalized_license_text() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("LICENSE"), "shared upstream notice\r\n").unwrap();
        let packages = vec![package(dir.path(), "b"), package(dir.path(), "a")];
        let text = render(&packages, dir.path(), &dir.path().join("supplements")).unwrap();
        assert_eq!(
            text,
            render(&packages.into_iter().rev().collect::<Vec<_>>(), dir.path(), &dir.path().join("supplements"))
                .unwrap()
        );
        assert_eq!(text.matches("shared upstream notice").count(), 1);
        assert!(text.contains("a 1.0.0") && text.contains("b 1.0.0"));
    }

    #[test]
    #[cfg(unix)]
    fn licenses_cannot_escape_crate_via_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let external = tempfile::NamedTempFile::new().unwrap();
        std::os::unix::fs::symlink(external.path(), dir.path().join("LICENSE")).unwrap();
        assert!(license_files(&package(dir.path(), "fixture"), &dir.path().join("absent")).is_err());
    }
}
