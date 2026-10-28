use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsStr,
    fs,
    io::{BufRead, BufReader, Write},
    net::{Shutdown, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
const HELPERS: [(&str, &str); 2] = [("i-chatgpt-plugin", "responses"), ("i-claude-plugin", "anthropic")];
fn valid_manifest(value: &Value, name: &str, protocol: &str) -> bool {
    let id = if name == "i-chatgpt-plugin" { "openai.chatgpt" } else { "anthropic.claude" };
    value["format_version"] == 1
        && value["kind"] == "connection"
        && value["id"] == id
        && value["protocol"] == protocol
        && value["name"].as_str().is_some_and(|name| !name.trim().is_empty())
        && value.as_object().is_some_and(|object| object.len() == 5)
}
fn suffix() -> &'static str {
    if cfg!(windows) { ".exe" } else { "" }
}
fn binary(name: &str) -> PathBuf {
    crate::root().join("target/debug").join(format!("{name}{}", suffix()))
}
pub fn fake_ant(args: &[String]) -> Result<()> {
    let root = PathBuf::from(std::env::var_os("ANTHROPIC_CONFIG_DIR").unwrap());
    fs::create_dir_all(&root)?;
    fs::write(root.join("args"), args.join("\n"))?;
    for name in ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "OPENAI_API_KEY"] {
        if std::env::var_os(name).is_some() {
            fs::write(root.join("inherited-secret"), name)?;
        }
    }
    ensure!(std::env::var("ANTHROPIC_BASE_URL")? == "https://api.anthropic.com", "Wrong fixed provider endpoint");
    ensure!(args.ends_with(&["--profile".into(), "i".into()]), "Wrong isolated profile");
    match args.get(1).map(String::as_str) {
        Some("login") => {
            fs::write(root.join("login-started"), std::process::id().to_string())?;
            while root.join("slow").exists() {
                thread::sleep(Duration::from_millis(20));
            }
            fs::write(root.join("signed-in"), "fixture")?;
        }
        Some("print-credentials") => {
            ensure!(root.join("signed-in").exists() && !root.join("fail").exists(), "Signed out");
            if root.join("oversized").exists() {
                print!("{}", "x".repeat(70000));
            } else {
                println!("SYNTHETIC_ACCESS_TOKEN");
            }
        }
        Some("logout") => {
            let _ = fs::remove_file(root.join("signed-in"));
        }
        _ => anyhow::bail!("Unknown fake ant command"),
    };
    Ok(())
}
struct Helper {
    child: Child,
    url: String,
    token: String,
}
impl Drop for Helper {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Helper {
    fn start(name: &str, root: &Path, extra_path: Option<&Path>) -> Result<Self> {
        let mut cmd = Command::new(binary(name));
        crate::isolated(&mut cmd, root);
        if let Some(path) = extra_path {
            let mut paths = vec![path.to_path_buf()];
            paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()));
            cmd.env("PATH", std::env::join_paths(paths)?)
                .env("ANTHROPIC_API_KEY", "INHERITED_FIXTURE_TOKEN")
                .env("OPENAI_API_KEY", "INHERITED_FIXTURE_TOKEN");
        }
        cmd.args(["--data-dir", root.to_str().unwrap(), "serve", "--origin", "https://i.lninl.com"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let child = cmd.spawn()?;
        let mut helper = Self { child, url: String::new(), token: String::new() };
        let stdout = helper.child.stdout.take().unwrap();
        let (send, receive) = std::sync::mpsc::channel();
        thread::spawn(move || {
            let mut stdout = BufReader::new(stdout);
            let result = (|| -> Result<String> {
                let mut line = String::new();
                stdout.read_line(&mut line)?;
                ensure!(line.contains("pairing link"), "Missing pairing link");
                line.clear();
                stdout.read_line(&mut line)?;
                Ok(line)
            })();
            let _ = send.send(result);
        });
        let line = receive
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| anyhow::anyhow!("Helper startup timed out"))??;
        let encoded =
            line.trim().split_once("#connection-plugin=").ok_or_else(|| anyhow::anyhow!("Invalid pairing link"))?.1;
        let pair: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(encoded)?)?;
        helper.url = pair["url"].as_str().ok_or_else(|| anyhow::anyhow!("Invalid helper URL"))?.into();
        helper.token = pair["token"].as_str().ok_or_else(|| anyhow::anyhow!("Invalid helper capability"))?.into();
        Ok(helper)
    }
    fn request(&self, path: &str, body: Option<Value>) -> Result<(u16, Value)> {
        let http = reqwest::blocking::Client::builder().no_proxy().timeout(Duration::from_secs(12)).build()?;
        let request = if let Some(body) = body {
            http.post(format!("{}{path}", self.url)).json(&body)
        } else {
            http.get(format!("{}{path}", self.url))
        };
        let response = request.header("Origin", "https://i.lninl.com").header("X-I-Local-Token", &self.token).send()?;
        let status = response.status().as_u16();
        Ok((status, response.json()?))
    }
}
fn until(mut condition: impl FnMut() -> bool) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        ensure!(Instant::now() < deadline, "Fixture condition timed out");
        thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}
#[cfg(unix)]
fn stopped(pid: i32) -> bool {
    unsafe { libc::kill(pid, 0) != 0 }
}
#[cfg(windows)]
fn stopped(pid: i32) -> bool {
    let output = Command::new("tasklist").args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"]).output();
    output.is_ok_and(|o| !String::from_utf8_lossy(&o.stdout).contains(&format!("\"{pid}\"")))
}
pub fn check() -> Result<()> {
    for (name, protocol) in HELPERS {
        let temp = tempfile::tempdir()?;
        let root = temp.path().canonicalize()?;
        let helper = Helper::start(name, &root, None)?;
        let (status, manifest) = helper.request("/v1/manifest", None)?;
        ensure!(status == 200 && valid_manifest(&manifest, name, protocol), "Invalid helper manifest");
        ensure!(!manifest.to_string().contains(&helper.token), "Capability leaked in manifest");
        let http = reqwest::blocking::Client::builder().no_proxy().timeout(Duration::from_secs(12)).build()?;
        for (header, value) in [
            ("Origin", "https://untrusted.invalid"),
            ("X-I-Local-Token", "wrong"),
            ("Host", "rebinding.invalid"),
            ("Origin", "null"),
        ] {
            let mut request = http
                .get(format!("{}/v1/manifest", helper.url))
                .header("Origin", if header == "Origin" { value } else { "https://i.lninl.com" })
                .header("X-I-Local-Token", if header == "X-I-Local-Token" { value } else { &helper.token });
            if header == "Host" {
                request = request.header(header, value);
            }
            let response = request.send()?;
            ensure!(
                response.status() == 403 && response.headers().get("access-control-allow-origin").is_none(),
                "Helper accepted an unauthorized {header} caller (status {})",
                response.status()
            );
        }
        let response = http
            .request(reqwest::Method::OPTIONS, format!("{}/v1/manifest", helper.url))
            .header("Origin", "https://i.lninl.com")
            .send()?;
        ensure!(
            response.status() == 204 && response.headers()["access-control-allow-private-network"] == "true",
            "Invalid preflight"
        );
        ensure!(
            helper.request("/v1/login", Some(json!({"unknown":"must not open browser"})))?.0 == 400,
            "Invalid login was not rejected"
        );
        ensure!(helper.request("/v1/cancel", Some(json!({})))?.0 == 200, "Cancel failed");
        drop(helper);
        let config = root.join("connection.json");
        let mut cmd = Command::new(binary(name));
        crate::isolated(&mut cmd, &root);
        cmd.args([
            "--data-dir",
            root.join("private").to_str().unwrap(),
            "configure",
            "--model",
            "offline-fixture",
            "--output",
            config.to_str().unwrap(),
        ]);
        ensure!(crate::run(&mut cmd, b"")?.status.success(), "Configure failed");
        let original = fs::read(&config)?;
        let config_value: Value = serde_json::from_slice(&original)?;
        ensure!(
            config_value["provider"]["completion"] == "plugin"
                && config_value["provider"]["exec"].as_array().unwrap().contains(&json!("--data-dir")),
            "Invalid worker configuration"
        );
        ensure!(
            !String::from_utf8_lossy(&original).contains("refresh_token") && config_value.get("api_key").is_none(),
            "Credentials entered configuration"
        );
        ensure!(
            !crate::run(&mut cmd, b"")?.status.success() && fs::read(&config)? == original,
            "Configure overwrote a saved file"
        );
    }
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    let fake = root.join(format!("ant{}", suffix()));
    fs::copy(std::env::current_exe()?, &fake)?;
    let private = root.join("private");
    let helper = Helper::start("i-claude-plugin", &private, Some(&root))?;
    let ant = private.join("ant");
    fs::create_dir_all(&ant)?;
    fs::write(ant.join("slow"), b"")?;
    let address = helper.url.trim_start_matches("http://");
    type PendingLogin = thread::JoinHandle<Result<()>>;
    let login = |close: bool| -> Result<Option<PendingLogin>> {
        let mut socket = TcpStream::connect(address)?;
        write!(
            socket,
            "POST /v1/login HTTP/1.1\r\nHost: {address}\r\nOrigin: https://i.lninl.com\r\nX-I-Local-Token: {}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{{}}",
            helper.token
        )?;
        until(|| ant.join("login-started").exists())?;
        if close {
            socket.shutdown(Shutdown::Both)?;
            Ok(None)
        } else {
            Ok(Some(thread::spawn(move || {
                use std::io::Read;
                socket.set_read_timeout(Some(Duration::from_secs(12)))?;
                let mut bytes = vec![];
                socket.read_to_end(&mut bytes)?;
                ensure!(String::from_utf8_lossy(&bytes).starts_with("HTTP/1.1 400"), "Cancelled login was accepted");
                Ok(())
            })))
        }
    };
    let pending = login(false)?.unwrap();
    ensure!(helper.request("/v1/cancel", Some(json!({})))?.0 == 200, "Cancel failed");
    pending.join().unwrap()?;
    let pid: i32 = fs::read_to_string(ant.join("login-started"))?.parse()?;
    until(|| stopped(pid))?;
    ensure!(!ant.join("signed-in").exists(), "Cancelled login committed");
    fs::remove_file(ant.join("login-started"))?;
    login(true)?;
    let pid: i32 = fs::read_to_string(ant.join("login-started"))?.parse()?;
    until(|| stopped(pid))?;
    ensure!(!ant.join("signed-in").exists(), "Disconnected login committed");
    fs::remove_file(ant.join("slow"))?;
    let result = helper.request("/v1/login", Some(json!({})))?;
    ensure!(result.0 == 200 && result.1["signed_in"] == true, "Login permit was retained after cancellation");
    ensure!(!ant.join("inherited-secret").exists(), "ant inherited caller credentials");
    let result = helper.request("/v1/status", None)?;
    ensure!(!result.1.to_string().contains("SYNTHETIC_ACCESS_TOKEN"), "Token leaked in status");
    fs::write(ant.join("oversized"), b"")?;
    ensure!(helper.request("/v1/status", None)?.1["accounts"][0]["signed_in"] == false, "Oversized token was accepted");
    fs::remove_file(ant.join("oversized"))?;
    ensure!(
        helper.request("/v1/logout", Some(json!({})))?.1["signed_out"] == true && private.join("signed-out").exists(),
        "Logout was not persisted"
    );
    ensure!(
        helper.request("/v1/status", None)?.1["accounts"][0]["signed_in"] == false,
        "Signed out credentials reappeared"
    );
    println!("PASS: native helper boundaries, configuration, process cancellation/disconnect and logout");
    Ok(())
}
pub fn package(args: &[String]) -> Result<()> {
    let profile = args.windows(2).find(|v| v[0] == "--profile").map(|v| v[1].as_str()).unwrap_or("min-size");
    ensure!(["debug", "release", "min-size"].contains(&profile), "Invalid profile");
    let host = Command::new("rustc").arg("-vV").output()?;
    ensure!(host.status.success(), "Cannot detect helper target");
    let host = String::from_utf8(host.stdout)?;
    let host =
        host.lines().find_map(|l| l.strip_prefix("host: ")).ok_or_else(|| anyhow::anyhow!("Missing host triple"))?;
    let encoded = std::env::var_os("CARGO_ENCODED_RUSTFLAGS");
    let flags = std::env::var_os("RUSTFLAGS");
    let mut plan = helper_build_plan(
        &crate::root(),
        host,
        profile,
        args.iter().any(|arg| arg == "--skip-build"),
        encoded.as_deref(),
        flags.as_deref(),
    )?;
    if let Some(command) = plan.command.as_mut() {
        ensure!(command.status()?.success(), "Connection helper build failed");
    }
    let helpers: Vec<_> =
        HELPERS.iter().map(|(name, _)| (*name, plan.binaries_dir.join(format!("{name}{}", suffix())))).collect();
    let out = args.windows(2).find(|v| v[0] == "--output").map(|v| PathBuf::from(&v[1]))
        .unwrap_or_else(|| crate::root().join("dist"));
    let archive = package_into(&out, host, &helpers, plan.rustc_flags.as_deref())?;
    if args.iter().any(|arg| arg == "--verify") {
        verify(&archive)?;
    }
    println!("{}", archive.display());
    Ok(())
}

struct HelperBuildPlan {
    command: Option<Command>,
    binaries_dir: PathBuf,
    rustc_flags: Option<Vec<String>>,
}

fn dynamic_crt_library(argument: &str) -> bool {
    let lower = argument.to_ascii_lowercase();
    let name = lower.trim_matches('"').trim_start_matches(['-', '/']);
    if name.starts_with("nodefaultlib:") {
        return false;
    }
    let name = name.strip_prefix("defaultlib:").unwrap_or(name);
    let name = name.rsplit(['\\', '/']).next().unwrap_or(name).trim_matches('"');
    name.starts_with("msvcr")
        || name.starts_with("vcruntime")
        || name.starts_with("msvcp")
        || name.starts_with("concrt")
        || matches!(name, "ucrt" | "ucrt.lib" | "ucrtbase" | "ucrtbase.dll" | "ucrtd" | "ucrtd.lib")
}

fn helper_build_plan(
    root: &Path,
    host: &str,
    profile: &str,
    skip: bool,
    encoded: Option<&OsStr>,
    plain: Option<&OsStr>,
) -> Result<HelperBuildPlan> {
    let msvc = host.ends_with("-windows-msvc");
    ensure!(!msvc || !skip, "MSVC connection packages require a fresh static CRT build; --skip-build is unsupported");
    let binaries_dir =
        if msvc { root.join("target").join(host).join(profile) } else { root.join("target").join(profile) };
    if skip {
        return Ok(HelperBuildPlan { command: None, binaries_dir, rustc_flags: None });
    }
    let mut command = Command::new("cargo");
    command.current_dir(root).args(["build", "--locked", "-p", "i-chatgpt", "--bins"]);
    if profile != "debug" {
        command.args(["--profile", profile]);
    }
    let rustc_flags = if msvc {
        let mut flags = if let Some(encoded) = encoded {
            let encoded = encoded.to_str().ok_or_else(|| anyhow::anyhow!("CARGO_ENCODED_RUSTFLAGS must be Unicode"))?;
            if encoded.is_empty() { vec![] } else { encoded.split('\u{1f}').map(str::to_owned).collect() }
        } else if let Some(plain) = plain {
            // Cargo splits RUSTFLAGS on whitespace, without interpreting shell quotes.
            let plain = plain.to_str().ok_or_else(|| anyhow::anyhow!("RUSTFLAGS must be Unicode"))?;
            ensure!(
                !plain.contains('\u{1f}'),
                "RUSTFLAGS cannot contain the encoded separator; use CARGO_ENCODED_RUSTFLAGS"
            );
            plain.split_whitespace().map(str::to_owned).collect()
        } else {
            vec![]
        };
        for (index, flag) in flags.iter().enumerate() {
            let codegen = if flag == "-C" || flag == "--codegen" {
                flags.get(index + 1).map(String::as_str)
            } else {
                flag.strip_prefix("-C")
                    .map(|flag| flag.trim_start_matches('='))
                    .or_else(|| flag.strip_prefix("--codegen="))
            };
            if let Some(codegen) = codegen {
                if let Some(features) = codegen.strip_prefix("target-feature=") {
                    ensure!(
                        !features.split(',').any(|feature| feature == "-crt-static"),
                        "Dynamic CRT flags conflict with portable MSVC helpers"
                    );
                }
                if let Some(arguments) =
                    codegen.strip_prefix("link-arg=").or_else(|| codegen.strip_prefix("link-args="))
                {
                    ensure!(
                        !arguments.split_whitespace().any(dynamic_crt_library),
                        "Dynamic CRT linker flags conflict with portable MSVC helpers"
                    );
                }
            }
            let library = if flag == "-l" { flags.get(index + 1).map(String::as_str) } else { flag.strip_prefix("-l") };
            if let Some(library) = library {
                ensure!(
                    !dynamic_crt_library(library.rsplit('=').next().unwrap_or(library)),
                    "Dynamic CRT libraries conflict with portable MSVC helpers"
                );
            }
        }
        flags.extend(["-C".into(), "target-feature=+crt-static".into()]);
        command.args(["--target", host, "--target-dir"]).arg(root.join("target"));
        command.env("CARGO_ENCODED_RUSTFLAGS", flags.join("\u{1f}")).env_remove("RUSTFLAGS");
        Some(flags)
    } else {
        None
    };
    Ok(HelperBuildPlan { command: Some(command), binaries_dir, rustc_flags })
}

fn package_into(
    out: &Path,
    target: &str,
    helpers: &[(&str, PathBuf)],
    rustc_flags: Option<&[String]>,
) -> Result<PathBuf> {
    ensure!(
        !target.is_empty() && target.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')),
        "Invalid target triple"
    );
    let expected: std::collections::HashSet<_> = HELPERS.iter().map(|(name, _)| *name).collect();
    let actual: std::collections::HashSet<_> = helpers.iter().map(|(name, _)| *name).collect();
    ensure!(helpers.len() == HELPERS.len() && actual == expected, "Both connection helpers are required");
    let mut contents = Vec::new();
    for (name, _) in HELPERS {
        let path = &helpers.iter().find(|(candidate, _)| *candidate == name).unwrap().1;
        ensure!(
            path.is_file() && !fs::symlink_metadata(path)?.file_type().is_symlink(),
            "Helper must be a regular file"
        );
        contents.push((name, fs::read(path)?));
    }
    fs::create_dir_all(out)?;
    let archive = out.join(format!("i-connections-{target}.zip"));
    let checksum = archive.with_extension("zip.sha256");
    ensure!(!archive.exists() && !checksum.exists(), "Connection package already exists");
    let file = fs::OpenOptions::new().write(true).create_new(true).open(&archive)?;
    let mut checksum_file = fs::OpenOptions::new().write(true).create_new(true).open(&checksum)?;
    let mut zip = zip::ZipWriter::new(file);
    let option = zip::write::SimpleFileOptions::default()
        .system(zip::System::Unix)
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default());
    let suffix = if target.contains("windows") { ".exe" } else { "" };
    for (name, bytes) in contents {
        zip.start_file(format!("bin/{name}{suffix}"), option.unix_permissions(0o755))?;
        zip.write_all(&bytes)?;
    }
    for (name, bytes) in [
        ("LICENSE", fs::read(crate::root().join("LICENSE"))?),
        ("README.txt", b"Optional trusted local connection helpers, managed through i.\n\nCLAUDE BILLING: OAuth avoids manual API-key entry, but uses Claude Console API billing. It does NOT use Claude Pro/Max plan allowances.\n\nRun i, then /plugins to install and enable, then /login to authorize and select a model.\nCommand line: i plugins install chatgpt; i plugins enable chatgpt; i login chatgpt.\nClaude Console: install official ant, then use the same i commands with claude.\nBrowser: i plugins serve chatgpt --origin https://i.lninl.com and open the private pairing link; do not share it. Credentials are not included.\nThese helper executables implement internal worker/service contracts, not a WASM sandbox.\nGuide: https://i.lninl.com/docs/chatgpt-login/\n".to_vec()),
        ("BUILD.json", json!({"format_version":1,"kind":"optional-connections","target":target,"crt_linkage":rustc_flags.map(|_|"static"),"rustc_flags":rustc_flags}).to_string().into_bytes()),
    ] {
        zip.start_file(name, option.unix_permissions(0o644))?;
        zip.write_all(&bytes)?;
    }
    zip.start_file("THIRD_PARTY_LICENSES.txt", option.unix_permissions(0o644))?;
    zip.write_all(&fs::read(crate::root().join("THIRD_PARTY_LICENSES.txt"))?)?;
    zip.finish()?;
    writeln!(
        checksum_file,
        "{:x}  {}",
        Sha256::digest(fs::read(&archive)?),
        archive.file_name().unwrap().to_string_lossy()
    )?;
    Ok(archive)
}

fn verify(archive: &Path) -> Result<()> {
    let mut zip = zip::ZipArchive::new(fs::File::open(archive)?)?;
    ensure!(zip.len() == 6, "Unexpected package members");
    let metadata: Value = serde_json::from_reader(zip.by_name("BUILD.json")?)?;
    ensure!(metadata["format_version"] == 1 && metadata["kind"] == "optional-connections", "Invalid package metadata");
    let target = metadata["target"].as_str().ok_or_else(|| anyhow::anyhow!("Invalid package target"))?;
    let suffix = if target.contains("windows") { ".exe" } else { "" };
    let expected: std::collections::HashSet<_> =
        ["BUILD.json".to_owned(), "README.txt".to_owned(), "LICENSE".to_owned(), "THIRD_PARTY_LICENSES.txt".to_owned()]
            .into_iter()
            .chain(HELPERS.iter().map(|(name, _)| format!("bin/{name}{suffix}")))
            .collect();
    let mut seen = std::collections::HashSet::new();
    let mut total = 0u64;
    for index in 0..zip.len() {
        let item = zip.by_index(index)?;
        let name = item.name()?.to_string();
        ensure!(
            expected.contains(&name) && seen.insert(name) && !item.is_symlink() && !item.encrypted(),
            "Unexpected package path"
        );
        total = total.checked_add(item.size()).ok_or_else(|| anyhow::anyhow!("Package size overflow"))?;
        ensure!(total <= 256 * 1024 * 1024, "Connection package too large");
    }
    let tmp = tempfile::tempdir()?;
    let root = tmp.path().canonicalize()?;
    for (name, protocol) in HELPERS {
        let executable = root.join(format!("{name}{suffix}"));
        let mut output = fs::File::create(&executable)?;
        std::io::copy(&mut zip.by_name(&format!("bin/{name}{suffix}"))?, &mut output)?;
        drop(output);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))?;
        }
        let mut cmd = Command::new(executable);
        crate::isolated(&mut cmd, &root);
        cmd.current_dir(&root).args(["--data-dir", root.join("state").join(name).to_str().unwrap(), "manifest"]);
        let result = crate::run(&mut cmd, b"")?;
        ensure!(result.status.success(), "Packaged helper failed");
        let value: Value = serde_json::from_slice(&result.stdout)?;
        ensure!(valid_manifest(&value, name, protocol), "Invalid packaged helper");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn command_args(command: &Command) -> Vec<String> {
        command.get_args().map(|arg| arg.to_str().unwrap().to_owned()).collect()
    }
    #[test]
    fn msvc_build_preserves_encoded_flags_and_separates_host_dependencies() {
        let root = Path::new("fixture-root");
        let original = "-C\u{1f}debuginfo=1\u{1f}--cfg\u{1f}fixture=\"value with spaces\"";
        for host in ["x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc"] {
            let plan = helper_build_plan(
                root,
                host,
                "min-size",
                false,
                Some(original.as_ref()),
                Some("-C target-feature=-crt-static".as_ref()),
            )
            .unwrap();
            let command = plan.command.as_ref().unwrap();
            let args = command_args(command);
            assert!(args.windows(2).any(|pair| pair == ["--target", host]));
            let target_dir = root.join("target");
            assert!(args.windows(2).any(|pair| pair[0] == "--target-dir" && pair[1] == target_dir.to_str().unwrap()));
            assert!(args.windows(2).any(|pair| pair == ["--profile", "min-size"]));
            assert_eq!(plan.binaries_dir, root.join("target").join(host).join("min-size"));
            let expected = format!("{original}\u{1f}-C\u{1f}target-feature=+crt-static");
            assert!(
                command
                    .get_envs()
                    .any(|(key, value)| key == "CARGO_ENCODED_RUSTFLAGS" && value == Some(expected.as_ref()))
            );
            assert!(command.get_envs().any(|(key, value)| key == "RUSTFLAGS" && value.is_none()));
            assert_eq!(
                plan.rustc_flags.unwrap(),
                vec!["-C", "debuginfo=1", "--cfg", "fixture=\"value with spaces\"", "-C", "target-feature=+crt-static"]
            );
        }
    }
    #[test]
    fn msvc_build_converts_plain_flags_using_cargos_whitespace_contract() {
        let plain = "-C\tdebuginfo=1\n--cfg fixture=\"one\"";
        let plan =
            helper_build_plan(Path::new("root"), "x86_64-pc-windows-msvc", "debug", false, None, Some(plain.as_ref()))
                .unwrap();
        assert_eq!(
            plan.rustc_flags.unwrap(),
            vec!["-C", "debuginfo=1", "--cfg", "fixture=\"one\"", "-C", "target-feature=+crt-static"]
        );
        let args = command_args(plan.command.as_ref().unwrap());
        assert!(!args.iter().any(|arg| arg == "--profile"));
        assert!(args.windows(2).any(|pair| pair == ["--target", "x86_64-pc-windows-msvc"]));
    }
    #[test]
    fn msvc_build_rejects_dynamic_crt_flags_and_unverifiable_skip_build() {
        for encoded in [
            "-C\u{1f}target-feature=-crt-static",
            "-Ctarget-feature=+sse2,-crt-static",
            "-C=target-feature=-crt-static",
            "--codegen=target-feature=-crt-static",
            "-C\u{1f}link-arg=/DEFAULTLIB:MSVCRT.lib",
            "-C\u{1f}link-arg=VCRUNTIME.lib",
            "-l\u{1f}dylib=ucrt",
        ] {
            assert!(
                helper_build_plan(
                    Path::new("root"),
                    "x86_64-pc-windows-msvc",
                    "release",
                    false,
                    Some(encoded.as_ref()),
                    None
                )
                .is_err(),
                "{encoded:?}"
            );
        }
        assert!(helper_build_plan(Path::new("root"), "x86_64-pc-windows-msvc", "release", true, None, None).is_err());
    }
    #[test]
    fn msvc_flags_handle_empty_encoded_precedence_and_safe_static_link_directives() {
        let empty = helper_build_plan(
            Path::new("root"),
            "x86_64-pc-windows-msvc",
            "release",
            false,
            Some("".as_ref()),
            Some("-C target-feature=-crt-static".as_ref()),
        )
        .unwrap();
        assert_eq!(empty.rustc_flags.unwrap(), ["-C", "target-feature=+crt-static"]);
        let allowed = "-C\u{1f}link-arg=/NODEFAULTLIB:MSVCRT.lib\u{1f}-l\u{1f}static=libcmt";
        assert!(
            helper_build_plan(
                Path::new("root"),
                "x86_64-pc-windows-msvc",
                "release",
                false,
                Some(allowed.as_ref()),
                None
            )
            .is_ok()
        );
        assert!(
            helper_build_plan(
                Path::new("root"),
                "x86_64-pc-windows-msvc",
                "release",
                false,
                None,
                Some("--cfg\u{1f}injected".as_ref())
            )
            .is_err()
        );
    }
    #[test]
    fn non_msvc_preserves_existing_build_and_skip_paths_without_claiming_static_crt() {
        for skip in [false, true] {
            let plan = helper_build_plan(
                Path::new("root"),
                "aarch64-apple-darwin",
                "release",
                skip,
                Some("-C\u{1f}target-feature=-crt-static".as_ref()),
                None,
            )
            .unwrap();
            assert_eq!(plan.binaries_dir, Path::new("root/target/release"));
            assert!(plan.rustc_flags.is_none());
            assert_eq!(plan.command.is_none(), skip);
            if let Some(command) = plan.command {
                assert!(!command_args(&command).iter().any(|arg| arg == "--target"));
            }
        }
    }
    #[test]
    fn connection_manifest_must_match_the_exact_public_contract() {
        let valid = json!({"format_version":1,"kind":"connection","id":"openai.chatgpt","name":"ChatGPT","protocol":"responses"});
        assert!(valid_manifest(&valid, "i-chatgpt-plugin", "responses"));
        for (key, value) in [
            ("format_version", json!(2)),
            ("kind", json!("provider")),
            ("id", json!("wrong")),
            ("protocol", json!("anthropic")),
            ("name", json!(" ")),
        ] {
            let mut invalid = valid.clone();
            invalid[key] = value;
            assert!(!valid_manifest(&invalid, "i-chatgpt-plugin", "responses"), "{key}");
        }
        let mut invalid = valid.clone();
        invalid["access_token"] = json!("must not be public");
        assert!(!valid_manifest(&invalid, "i-chatgpt-plugin", "responses"));
        invalid.as_object_mut().unwrap().remove("name");
        assert!(!valid_manifest(&invalid, "i-chatgpt-plugin", "responses"));
    }
    fn fixtures(root: &Path) -> Vec<(&'static str, PathBuf)> {
        HELPERS
            .iter()
            .map(|(name, _)| {
                let path = root.join(name);
                fs::write(&path, b"fixture executable").unwrap();
                (*name, path)
            })
            .collect()
    }
    #[test]
    fn fresh_msvc_metadata_records_requested_static_crt_and_all_effective_flags() {
        let temp = tempfile::tempdir().unwrap();
        let host = "x86_64-pc-windows-msvc";
        let plan = helper_build_plan(
            temp.path(),
            host,
            "min-size",
            false,
            Some("--cfg\u{1f}fixture=\"build flags\"".as_ref()),
            None,
        )
        .unwrap();
        let archive =
            package_into(&temp.path().join("out"), host, &fixtures(temp.path()), plan.rustc_flags.as_deref()).unwrap();
        let mut zip = zip::ZipArchive::new(fs::File::open(archive).unwrap()).unwrap();
        let metadata: Value = serde_json::from_reader(zip.by_name("BUILD.json").unwrap()).unwrap();
        assert_eq!(metadata["crt_linkage"], "static");
        assert_eq!(
            metadata["rustc_flags"],
            json!(["--cfg", "fixture=\"build flags\"", "-C", "target-feature=+crt-static"])
        );
    }
    #[test]
    fn connection_archives_are_fixed_deterministic_and_never_include_credentials() {
        let temp = tempfile::tempdir().unwrap();
        let helpers = fixtures(temp.path());
        fs::write(temp.path().join("accounts.json"), "DO_NOT_PACKAGE_CREDENTIAL_FIXTURE").unwrap();
        for target in ["aarch64-apple-darwin", "x86_64-pc-windows-msvc"] {
            let archive = package_into(&temp.path().join(target), target, &helpers, None).unwrap();
            let before = fs::read(&archive).unwrap();
            let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&before)).unwrap();
            let suffix = if target.contains("windows") { ".exe" } else { "" };
            let expected: std::collections::HashSet<_> = [
                "LICENSE".to_owned(),
                "README.txt".to_owned(),
                "BUILD.json".to_owned(),
                "THIRD_PARTY_LICENSES.txt".to_owned(),
            ]
            .into_iter()
            .chain(HELPERS.iter().map(|(name, _)| format!("bin/{name}{suffix}")))
            .collect();
            let mut actual = std::collections::HashSet::new();
            for i in 0..zip.len() {
                let mut item = zip.by_index(i).unwrap();
                let name = item.name().unwrap().to_string();
                actual.insert(name.clone());
                let mut bytes = vec![];
                std::io::Read::read_to_end(&mut item, &mut bytes).unwrap();
                assert!(!String::from_utf8_lossy(&bytes).contains("DO_NOT_PACKAGE"));
                assert!(!item.is_symlink());
                if name.starts_with("bin/") {
                    assert_eq!(item.unix_mode().unwrap() & 0o777, 0o755);
                }
            }
            assert_eq!(actual, expected);
            let mut notices = String::new();
            std::io::Read::read_to_string(&mut zip.by_name("THIRD_PARTY_LICENSES.txt").unwrap(), &mut notices).unwrap();
            assert_eq!(notices, fs::read_to_string(crate::root().join("THIRD_PARTY_LICENSES.txt")).unwrap());
            let metadata: Value = serde_json::from_reader(zip.by_name("BUILD.json").unwrap()).unwrap();
            assert_eq!(
                metadata,
                json!({"format_version":1,"kind":"optional-connections","target":target,"crt_linkage":null,"rustc_flags":null})
            );
            let checksum = archive.with_extension("zip.sha256");
            let original_checksum = fs::read(&checksum).unwrap();
            assert_eq!(
                String::from_utf8(original_checksum.clone()).unwrap().split_whitespace().next().unwrap(),
                format!("{:x}", Sha256::digest(&before))
            );
            assert!(package_into(archive.parent().unwrap(), target, &helpers, None).is_err());
            assert_eq!(fs::read(&archive).unwrap(), before);
            assert_eq!(fs::read(&checksum).unwrap(), original_checksum);
            let other = package_into(&temp.path().join(format!("other-{target}")), target, &helpers, None).unwrap();
            assert_eq!(fs::read(other).unwrap(), before);
        }
    }
    #[test]
    fn rejects_invalid_targets_missing_or_duplicate_helpers_and_existing_checksum() {
        let temp = tempfile::tempdir().unwrap();
        let helpers = fixtures(temp.path());
        for target in ["", "../escape", "/escape", "target:stream", "a\\b"] {
            assert!(package_into(&temp.path().join("out"), target, &helpers, None).is_err());
        }
        assert!(package_into(&temp.path().join("out"), "fixture", &[], None).is_err());
        let duplicate = vec![helpers[0].clone(), helpers[0].clone()];
        assert!(package_into(&temp.path().join("out"), "fixture", &duplicate, None).is_err());
        let mut missing = helpers.clone();
        missing[0].1 = temp.path().join("missing");
        assert!(package_into(&temp.path().join("out"), "fixture", &missing, None).is_err());
        let out = temp.path().join("out");
        fs::create_dir(&out).unwrap();
        let checksum = out.join("i-connections-fixture.zip.sha256");
        fs::write(&checksum, b"existing audited checksum").unwrap();
        assert!(package_into(&out, "fixture", &helpers, None).is_err());
        assert_eq!(fs::read(checksum).unwrap(), b"existing audited checksum");
        assert!(!out.join("i-connections-fixture.zip").exists());
    }
    #[cfg(unix)]
    #[test]
    fn connection_package_rejects_source_and_checksum_symlinks() {
        let temp = tempfile::tempdir().unwrap();
        let helpers = fixtures(temp.path());
        let link = temp.path().join("helper-link");
        std::os::unix::fs::symlink(&helpers[0].1, &link).unwrap();
        let mut linked = helpers.clone();
        linked[0].1 = link;
        assert!(package_into(&temp.path().join("out"), "fixture", &linked, None).is_err());
        let out = temp.path().join("checksum");
        fs::create_dir(&out).unwrap();
        let sentinel = temp.path().join("sentinel");
        fs::write(&sentinel, b"must stay").unwrap();
        std::os::unix::fs::symlink(&sentinel, out.join("i-connections-fixture.zip.sha256")).unwrap();
        assert!(package_into(&out, "fixture", &helpers, None).is_err());
        assert_eq!(fs::read(sentinel).unwrap(), b"must stay");
    }
    #[test]
    fn verification_rejects_unexpected_paths_and_links_before_running_helpers() {
        for fault in ["extra", "traversal", "case", "link"] {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("bad.zip");
            let mut writer = zip::ZipWriter::new(fs::File::create(&path).unwrap());
            let options = zip::write::SimpleFileOptions::default();
            writer.start_file("BUILD.json", options).unwrap();
            writer.write_all(br#"{"format_version":1,"kind":"optional-connections","target":"fixture"}"#).unwrap();
            for name in
                ["LICENSE", "README.txt", "THIRD_PARTY_LICENSES.txt", "bin/i-chatgpt-plugin", "bin/i-claude-plugin"]
            {
                let name = match (fault, name) {
                    ("traversal", "README.txt") => "../escape",
                    ("case", "README.txt") => "readme.txt",
                    _ => name,
                };
                if fault == "link" && name == "bin/i-chatgpt-plugin" {
                    writer.add_symlink(name, "../../escape", options).unwrap();
                } else {
                    writer.start_file(name, options).unwrap();
                    writer.write_all(b"not executable").unwrap();
                }
            }
            if fault == "extra" {
                writer.start_file("accounts.json", options).unwrap();
                writer.write_all(b"private").unwrap();
            }
            writer.finish().unwrap();
            let error = verify(&path).unwrap_err().to_string();
            assert!(error.contains("Unexpected package"), "{fault}: {error}");
            assert!(!temp.path().join("escape").exists());
        }
    }
    #[test]
    fn packaged_manifest_runs_only_in_an_isolated_directory() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("fixture.rs");
        fs::write(&source,r#"use std::{env,path::PathBuf};
fn main(){
 let home=PathBuf::from(env::var_os("HOME").unwrap());
 assert_eq!(env::current_dir().unwrap().canonicalize().unwrap(),home.canonicalize().unwrap());
 assert_eq!(PathBuf::from(env::var_os("USERPROFILE").unwrap()),home);
 for key in ["OPENAI_API_KEY","ANTHROPIC_API_KEY","ANTHROPIC_AUTH_TOKEN"]{assert!(env::var_os(key).is_none());}
 let args:Vec<String>=env::args().skip(1).collect();assert_eq!(args.len(),3);assert_eq!(args[0],"--data-dir");assert_eq!(args[2],"manifest");
 let state=PathBuf::from(&args[1]);assert!(state.is_absolute() && state.starts_with(home.join("state")));
 let name=env::current_exe().unwrap();let name=name.file_stem().unwrap().to_str().unwrap();
 let (id,protocol)=if name=="i-chatgpt-plugin" {("openai.chatgpt","responses")}else{assert_eq!(name,"i-claude-plugin");("anthropic.claude","anthropic")};
 println!("{{\"format_version\":1,\"kind\":\"connection\",\"id\":\"{id}\",\"name\":\"Fixture\",\"protocol\":\"{protocol}\"}}");
}"#).unwrap();
        let executable = temp.path().join(format!("fixture{}", suffix()));
        let result = crate::run(
            Command::new("rustc")
                .args(["--edition=2024", "--crate-name", "manifest_fixture"])
                .arg(&source)
                .arg("-o")
                .arg(&executable),
            b"",
        )
        .unwrap();
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
        let helpers: Vec<_> = HELPERS.iter().map(|(name, _)| (*name, executable.clone())).collect();
        let target = if cfg!(windows) { "fixture-windows" } else { "fixture-unix" };
        let archive = package_into(&temp.path().join("out"), target, &helpers, None).unwrap();
        verify(&archive).unwrap();
    }
}
