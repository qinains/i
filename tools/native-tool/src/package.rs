use anyhow::{Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

fn member(name: &str, seen: &mut HashSet<String>) -> Result<PathBuf> {
    ensure!(!name.is_empty() && !name.starts_with('/') && !name.contains(['\\', '\0', ':']), "Unsafe package member");
    let parts: Vec<_> = name.trim_end_matches('/').split('/').collect();
    ensure!(
        parts.iter().all(|p| !p.is_empty() && *p != "." && *p != ".." && !p.ends_with(['.', ' '])),
        "Unsafe package path"
    );
    ensure!(seen.insert(name.trim_end_matches('/').to_lowercase()), "Duplicate package member");
    Ok(parts.iter().collect())
}
pub fn extract(archive: &Path, out: &Path) -> Result<PathBuf> {
    ensure!(
        fs::symlink_metadata(out)?.is_dir() && fs::read_dir(out)?.next().is_none(),
        "Extraction requires an empty real directory"
    );
    let mut seen = HashSet::new();
    let mut total = 0u64;
    if archive.extension().is_some_and(|ext| ext == "zip") {
        let mut zip = zip::ZipArchive::new(fs::File::open(archive)?)?;
        ensure!(zip.len() <= 10000, "Too many entries");
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            let path = member(entry.name()?.as_ref(), &mut seen)?;
            ensure!(!entry.is_symlink() && !entry.encrypted(), "Unsupported package entry");
            total = total.checked_add(entry.size()).ok_or_else(|| anyhow::anyhow!("Package size overflow"))?;
            ensure!(total <= 512 * 1024 * 1024, "Package too large");
            let target = out.join(path);
            if entry.is_dir() {
                fs::create_dir_all(target)?;
            } else {
                fs::create_dir_all(target.parent().unwrap())?;
                let mut bytes = vec![];
                (&mut entry).take(512 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
                ensure!(bytes.len() as u64 == entry.size(), "Invalid member size");
                fs::write(&target, bytes)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(
                        target,
                        fs::Permissions::from_mode(entry.unix_mode().unwrap_or(0o644) & 0o777),
                    )?;
                }
            }
        }
    } else {
        let file = fs::File::open(archive)?;
        let reader: Box<dyn Read> = if archive.to_string_lossy().ends_with(".gz") {
            Box::new(flate2::read::GzDecoder::new(file))
        } else if archive.to_string_lossy().ends_with(".xz") {
            Box::new(lzma_rust2::XzReader::new_mem_limit(file, false, 128 * 1024))
        } else {
            Box::new(file)
        };
        let mut tar = tar::Archive::new(reader);
        for (count, entry) in tar.entries()?.enumerate() {
            ensure!(count < 10000, "Too many entries");
            let mut entry = entry?;
            let raw = entry.path_bytes();
            let name = std::str::from_utf8(&raw)?;
            let path = member(name, &mut seen)?;
            ensure!(
                entry.header().entry_type().is_file() || entry.header().entry_type().is_dir(),
                "Package links and special files are forbidden"
            );
            total = total.checked_add(entry.size()).ok_or_else(|| anyhow::anyhow!("Package size overflow"))?;
            ensure!(total <= 512 * 1024 * 1024, "Package too large");
            ensure!(entry.unpack_in(out)?, "Unsafe tar member: {}", path.display());
        }
    }
    let entries: Vec<_> = fs::read_dir(out)?.collect::<Result<_, _>>()?;
    ensure!(entries.len() == 1 && entries[0].file_type()?.is_dir(), "Package needs one bundle root");
    Ok(entries[0].path())
}
pub fn check(archive: &Path) -> Result<()> {
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    let extract_root = root.join("unpack");
    fs::create_dir(&extract_root)?;
    let bundle = extract(archive, &extract_root)?;
    ensure!(fs::read_to_string(bundle.join("LICENSE.txt"))?.contains("MIT License"), "Missing license");
    ensure!(
        fs::read(bundle.join("THIRD_PARTY_LICENSES.txt"))? == fs::read(crate::root().join("THIRD_PARTY_LICENSES.txt"))?,
        "Stale package notices"
    );
    for line in fs::read_to_string(bundle.join("SHA256SUMS"))?.lines() {
        let (digest, name) = line.split_once("  ").ok_or_else(|| anyhow::anyhow!("Invalid checksum line"))?;
        let path = member(name, &mut HashSet::new())?;
        ensure!(format!("{:x}", Sha256::digest(fs::read(bundle.join(path))?)) == digest, "Package checksum mismatch");
    }
    ensure!(
        fs::read_to_string(PathBuf::from(format!("{}.sha256", archive.display())))?.split_whitespace().next()
            == Some(format!("{:x}", Sha256::digest(fs::read(archive)?)).as_str()),
        "Archive checksum mismatch"
    );
    let metadata: Value = serde_json::from_slice(&fs::read(bundle.join("BUILD.json"))?)?;
    ensure!(metadata["layout"] == "bundled", "Expected bundled package");
    let executable = if cfg!(windows) { "i.exe" } else { "i" };
    let mut binary = bundle.join("bin").join(executable);
    ensure!(metadata["binary_total_bytes"].as_u64() == Some(fs::metadata(&binary)?.len()), "Binary size mismatch");
    ensure!(fs::read_dir(bundle.join("bin"))?.count() == 1, "Unexpected sidecars");
    let exec = |binary: &Path, args: &[String], input: &[u8]| -> Result<std::process::Output> {
        let mut command = Command::new(binary);
        crate::isolated(&mut command, &root);
        command.env("I_PACKAGE_TEST_KEY", "offline-fixture-only").current_dir(&root).args(args);
        let result = crate::run(&mut command, input)?;
        ensure!(result.status.success(), "Package command failed: {}", String::from_utf8_lossy(&result.stderr));
        Ok(result)
    };
    let args = |values: &[&str]| values.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    ensure!(
        String::from_utf8(exec(&binary, &args(&["--version"]), b"")?.stdout)?
            .contains(metadata["version"].as_str().unwrap_or("invalid")),
        "Version mismatch"
    );
    ensure!(
        String::from_utf8(exec(&binary, &args(&["--runtime", "logo", "--svg"]), b"")?.stdout)?.contains("cx=\"171.5\""),
        "Logo missing"
    );
    exec(&binary, &args(&["--runtime", "init"]), b"")?;
    let installed = root.join("installed/bin");
    exec(&binary, &["install".into(), "--bin-dir".into(), installed.to_string_lossy().into()], b"")?;
    fs::rename(&bundle, root.join("source-moved-away"))?;
    binary = installed.join(executable);
    let installed_binary = binary.canonicalize()?;
    let manifest: Value = serde_json::from_slice(&exec(&binary, &args(&["--runtime", "manifest"]), b"")?.stdout)?;
    for entry in manifest["tools"]
        .as_array()
        .unwrap()
        .iter()
        .chain(manifest["hooks"].as_array().unwrap())
        .chain(std::iter::once(&manifest["provider"]))
    {
        let binding = entry["exec"][0].as_str().ok_or_else(|| anyhow::anyhow!("Invalid executable binding"))?;
        ensure!(
            Path::new(binding).is_absolute()
                && fs::canonicalize(binding)? == installed_binary
                && entry["exec"][1] == "--runtime",
            "Nonportable binding"
        );
    }
    let workspace = root.join("project");
    fs::create_dir(&workspace)?;
    fs::write(workspace.join("unicode.txt"), "Hello 你好 Σ K\n")?;
    for (query, regex) in [("hello", false), ("σ", false), (r"\p{Script=Han}+", true)] {
        let frame = json!({"id":1,"event":"tool","workspace":workspace,"data":{"name":"search","arguments":{"query":query,"regex":regex}}});
        let reply: Value = serde_json::from_slice(
            &exec(&binary, &args(&["--runtime", "search"]), format!("{frame}\n").as_bytes())?.stdout,
        )?;
        let output: Value = serde_json::from_str(reply["data"]["output"].as_str().unwrap())?;
        ensure!(
            output["matches"].as_array().unwrap().len() == 1 && output["matches"][0]["path"] == "unicode.txt",
            "Unicode search failed"
        );
    }
    #[cfg(unix)]
    crate::tls::check(&binary, true)?;
    let server = Mock::start()?;
    fs::write(root.join("config/config.json"),json!({"protocol":"openai","base_url":format!("http://{}",server.address),"model":"package-fixture","api_key_env":"I_PACKAGE_TEST_KEY","max_tokens":64,"use_max_tokens":true,"plugins":"builtin.json","budget":{"context_window":16384}}).to_string())?;
    let result = exec(
        &binary,
        &[
            "--runtime".into(),
            "console".into(),
            "--no-logo".into(),
            format!("--dir={}", workspace.display()),
            "--session-name=check".into(),
        ],
        b"",
    )?;
    ensure!(String::from_utf8_lossy(&result.stderr).contains("Type a task"), "Console did not start");
    let result = exec(
        &binary,
        &[
            "--runtime".into(),
            "console".into(),
            "--no-logo".into(),
            "-C".into(),
            workspace.to_string_lossy().into(),
            "--max-steps".into(),
            "2".into(),
            "--dangerously-skip-permissions".into(),
            "--task=Remember the package_check preference as ok.".into(),
        ],
        b"",
    )?;
    ensure!(String::from_utf8_lossy(&result.stdout).contains("packaged-ok"), "Packaged model task failed");
    let requests = server.requests.lock().unwrap();
    ensure!(requests.len() == 2, "Unexpected model rounds");
    let schema: Value = serde_json::from_slice(&fs::read(crate::root().join("src/tools.json"))?)?;
    let expected: HashSet<String> = schema
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["function"]["name"].as_str().unwrap().into())
        .chain(manifest["tools"].as_array().unwrap().iter().map(|v| v["name"].as_str().unwrap().into()))
        .collect();
    let actual: Vec<String> = requests[0]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["function"]["name"].as_str().unwrap().into())
        .collect();
    ensure!(
        actual.len() == expected.len() && actual.into_iter().collect::<HashSet<_>>() == expected,
        "Tool schemas mismatch"
    );
    ensure!(requests[1]["messages"].as_array().unwrap().iter().any(|m| m["role"] == "tool"), "Missing tool result");
    drop(requests);
    let memory = glob::glob(root.join("data/workspaces/*/memory.json").to_str().unwrap())?
        .next()
        .ok_or_else(|| anyhow::anyhow!("Missing memory state"))??;
    let memory: Value = serde_json::from_slice(&fs::read(memory)?)?;
    ensure!(memory["package_check"] == "ok", "Bundled memory failed");
    let report: Value = serde_json::from_slice(
        &exec(
            &binary,
            &["--runtime".into(), "cache-report".into(), "-C".into(), workspace.to_string_lossy().into()],
            b"",
        )?
        .stdout,
    )?;
    ensure!(report["totals"]["observed_responses"] == 2, "Cache accounting failed");
    println!("PASS: {} integrity, relocation, workers, Unicode, model loop, memory and cache", archive.display());
    Ok(())
}
pub struct Mock {
    pub address: std::net::SocketAddr,
    pub requests: Arc<Mutex<Vec<Value>>>,
    stop: Arc<AtomicBool>,
    task: Option<thread::JoinHandle<()>>,
}
impl Mock {
    pub fn start() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let requests = Arc::new(Mutex::new(vec![]));
        let stop = Arc::new(AtomicBool::new(false));
        let shared = requests.clone();
        let stopped = stop.clone();
        let task = thread::spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                let (mut socket, _) = match listener.accept() {
                    Ok(v) => v,
                    Err(_) => {
                        thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                };
                socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                let mut header = vec![];
                loop {
                    let mut b = [0];
                    if socket.read_exact(&mut b).is_err() {
                        return;
                    }
                    header.push(b[0]);
                    if header.ends_with(b"\r\n\r\n") {
                        break;
                    }
                    assert!(header.len() < 16384);
                }
                let text = String::from_utf8(header).unwrap();
                let n: usize = text
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase().strip_prefix("content-length:").map(|n| n.trim().parse().unwrap())
                    })
                    .unwrap_or(0);
                let mut body = vec![0; n];
                socket.read_exact(&mut body).unwrap();
                let mut frames = shared.lock().unwrap();
                frames.push(serde_json::from_slice(&body).unwrap());
                let first = frames.len() == 1;
                let body=json!({"choices":[{"finish_reason":if first{"tool_calls"}else{"stop"},"message":if first{json!({"role":"assistant","content":null,"tool_calls":[{"id":"package-call","type":"function","function":{"name":"memory_set","arguments":"{\"key\":\"package_check\",\"value\":\"ok\"}"}}]})}else{json!({"role":"assistant","content":"packaged-ok"})}}],"usage":{"prompt_tokens":256,"prompt_tokens_details":{"cached_tokens":0},"completion_tokens":4}}).to_string();
                let _ = write!(
                    socket,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
            }
        });
        Ok(Self { address, requests, stop, task: Some(task) })
    }
}
impl Drop for Mock {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(task) = self.task.take() {
            let _ = task.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn archive(root: &Path, extension: &str, names: &[&str], link: bool) -> PathBuf {
        let path = root.join(format!("fixture.{extension}"));
        if extension == "zip" {
            let mut writer = zip::ZipWriter::new(fs::File::create(&path).unwrap());
            for name in names {
                if link {
                    writer.add_symlink(name, "../../escape", zip::write::SimpleFileOptions::default()).unwrap();
                } else {
                    writer.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
                    writer.write_all(b"bad").unwrap();
                }
            }
            writer.finish().unwrap();
        } else {
            let gz = flate2::write::GzEncoder::new(fs::File::create(&path).unwrap(), flate2::Compression::default());
            let mut writer = tar::Builder::new(gz);
            for name in names {
                let mut header = tar::Header::new_gnu();
                // Set raw names so the writer does not sanitize the hostile fixture.
                header.as_mut_bytes()[..name.len()].copy_from_slice(name.as_bytes());
                header.set_mode(0o644);
                if link {
                    header.set_entry_type(tar::EntryType::Symlink);
                    header.set_link_name("../../escape").unwrap();
                    header.set_size(0);
                } else {
                    header.set_size(3);
                }
                header.set_cksum();
                writer.append(&header, if link { &b""[..] } else { &b"bad"[..] }).unwrap();
            }
            writer.into_inner().unwrap().finish().unwrap();
        }
        path
    }
    #[test]
    fn zip_and_tar_reject_actual_traversal_links_case_collisions_and_multiple_roots() {
        for extension in ["zip", "tar.gz"] {
            for names in [
                vec!["../escape"],
                vec!["/escape"],
                vec!["C:/escape"],
                vec!["bundle/../escape"],
                vec!["bundle\\escape"],
                vec!["bundle/end."],
                vec!["bundle/end "],
                vec!["bundle/FILE", "bundle/file"],
                vec!["first/file", "second/file"],
            ] {
                let temp = tempfile::tempdir().unwrap();
                let input = archive(temp.path(), extension, &names, false);
                let out = temp.path().join("out");
                fs::create_dir(&out).unwrap();
                assert!(extract(&input, &out).is_err(), "accepted {extension}: {names:?}");
                assert!(!temp.path().join("escape").exists());
            }
            let temp = tempfile::tempdir().unwrap();
            let input = archive(temp.path(), extension, &["bundle/link"], true);
            let out = temp.path().join("out");
            fs::create_dir(&out).unwrap();
            assert!(extract(&input, &out).is_err(), "accepted {extension} link");
            assert!(!out.join("bundle/link").exists());
        }
    }
    #[test]
    fn extraction_never_overwrites_preexisting_files() {
        for extension in ["zip", "tar.gz"] {
            let temp = tempfile::tempdir().unwrap();
            let input = archive(temp.path(), extension, &["bundle/file"], false);
            let out = temp.path().join("out");
            fs::create_dir_all(out.join("bundle")).unwrap();
            fs::write(out.join("bundle/file"), b"audited existing file").unwrap();
            assert!(extract(&input, &out).is_err());
            assert_eq!(fs::read(out.join("bundle/file")).unwrap(), b"audited existing file");
        }
    }
    #[cfg(unix)]
    #[test]
    fn extraction_rejects_existing_directory_and_child_symlinks() {
        let temp = tempfile::tempdir().unwrap();
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("file"), b"keep").unwrap();
        for extension in ["zip", "tar.gz"] {
            let input = archive(temp.path(), extension, &["bundle/file"], false);
            let out = temp.path().join(extension);
            fs::create_dir(&out).unwrap();
            std::os::unix::fs::symlink(&outside, out.join("bundle")).unwrap();
            assert!(extract(&input, &out).is_err());
            assert_eq!(fs::read(outside.join("file")).unwrap(), b"keep");
            let linked = temp.path().join(format!("linked-{extension}"));
            std::os::unix::fs::symlink(&outside, &linked).unwrap();
            assert!(extract(&input, &linked).is_err());
        }
    }
    #[test]
    fn rejects_unsafe_and_duplicate_archive_names() {
        for path in
            ["../escape", "/escape", "C:/escape", "bundle/../escape", "bundle\\escape", "bundle/end.", "bundle/end "]
        {
            assert!(member(path, &mut HashSet::new()).is_err(), "{path}");
        }
        let mut seen = HashSet::new();
        assert!(member("bundle/FILE", &mut seen).is_ok());
        assert!(member("bundle/file", &mut seen).is_err());
    }
    #[test]
    fn zip_and_tar_layout_are_checked_without_extracting_links() {
        let temp = tempfile::tempdir().unwrap();
        for ext in ["zip", "tar.gz"] {
            let archive = temp.path().join(format!("fixture.{ext}"));
            if ext == "zip" {
                let mut z = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
                z.start_file("bundle/bin/i.exe", zip::write::SimpleFileOptions::default()).unwrap();
                z.write_all(b"fixture").unwrap();
                z.finish().unwrap();
            } else {
                let gz =
                    flate2::write::GzEncoder::new(fs::File::create(&archive).unwrap(), flate2::Compression::default());
                let mut tar = tar::Builder::new(gz);
                let mut h = tar::Header::new_gnu();
                h.set_size(7);
                h.set_mode(0o755);
                h.set_cksum();
                tar.append_data(&mut h, "bundle/bin/i.exe", &b"fixture"[..]).unwrap();
                tar.into_inner().unwrap().finish().unwrap();
            }
            let out = temp.path().join(ext);
            fs::create_dir(&out).unwrap();
            let bundle = extract(&archive, &out).unwrap();
            assert_eq!(fs::read(bundle.join("bin/i.exe")).unwrap(), b"fixture");
        }
    }
}
