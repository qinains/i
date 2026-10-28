//! Native connection management. No credentials enter the plugin selection file.
use super::{option, paths::AppPaths, state};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

const IDS: [&str; 2] = ["chatgpt", "claude"];
const LIMIT: usize = 64 * 1024 * 1024;
fn valid(id: &str) -> Result<()> {
    ensure!(IDS.contains(&id), "Choose chatgpt or claude");
    Ok(())
}
fn title(id: &str) -> &str {
    if id == "claude" { "Claude Console API (not Pro/Max)" } else { "ChatGPT" }
}
fn executable(id: &str) -> String {
    format!("i-{id}-plugin{}", std::env::consts::EXE_SUFFIX)
}
fn target() -> Result<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Ok("aarch64-apple-darwin"),
        ("linux", "x86_64") => Ok("x86_64-unknown-linux-gnu"),
        ("windows", "x86_64") => Ok("x86_64-pc-windows-msvc"),
        _ => bail!("No verified connection package for this platform; build the helpers from source"),
    }
}
fn selection(paths: &AppPaths) -> Result<Value> {
    let file = paths.config_dir.join("native-plugins.json");
    let value =
        if file.try_exists()? { serde_json::from_slice(&state::read_bounded(&file, 256_000)?)? } else { json!({}) };
    ensure!(value.is_object(), "Invalid native plugin selection");
    Ok(value)
}
fn save_selection(paths: &AppPaths, action: impl FnOnce(&mut Value) -> Result<()>) -> Result<()> {
    fs::create_dir_all(&paths.config_dir)?;
    let _guard = state::Lock::acquire(&paths.config_dir.join("native-plugins.lock"))?;
    let mut value = selection(paths)?;
    action(&mut value)?;
    state::atomic(&paths.config_dir.join("native-plugins.json"), &value)
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(super) fn checked_exe(paths: &AppPaths, id: &str, enabled: bool) -> Result<PathBuf> {
    valid(id)?;
    let value = selection(paths)?;
    let record = &value[id];
    let relative = record["executable"].as_str().context("Plugin is not installed; use /plugins install NAME")?;
    ensure!(!enabled || record["enabled"] == true, "Plugin is disabled; use /plugins enable {id}");
    let exe = state::inside(&paths.data_dir, Path::new(relative))?;
    ensure!(!exe.is_symlink(), "Plugin executable cannot be a symlink");
    ensure!(
        hash(&state::read_bounded(&exe, LIMIT)?) == record["sha256"].as_str().unwrap_or(""),
        "Plugin integrity check failed; reinstall it"
    );
    Ok(exe)
}
fn enable(paths: &AppPaths, id: &str, enabled: bool) -> Result<()> {
    valid(id)?;
    if enabled {
        checked_exe(paths, id, false)?;
    }
    save_selection(paths, |value| {
        ensure!(value.get(id).is_some(), "Plugin is not installed; use /plugins install NAME");
        value[id]["enabled"] = json!(enabled);
        Ok(())
    })
}
fn download_config(proxy: &super::proxy::Setting) -> Result<ureq::config::Config> {
    Ok(ureq::Agent::config_builder()
        .proxy(proxy.download_proxy()?)
        .https_only(true)
        .timeout_global(Some(Duration::from_secs(120)))
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_resolve(Some(Duration::from_secs(10)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .provider(ureq::tls::TlsProvider::NativeTls)
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .build())
}
fn download(url: &str, limit: usize, progress: &super::installation::Context) -> Result<Vec<u8>> {
    progress.check()?;
    let config = download_config(&progress.proxy)?;
    let mut response = ureq::Agent::new_with_config(config)
        .get(url)
        .header("User-Agent", "lninl-i-plugin-installer")
        .call()
        .context("Cannot download the official plugin release; check network/proxy")?;
    let total =
        response.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<usize>().ok());
    ensure!(total.is_none_or(|n| n <= limit), "Plugin download exceeds size limit");
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    let mut reported = 0;
    let mut reader = response.body_mut().as_reader();
    loop {
        progress.check()?;
        let n = reader
            .read(&mut chunk)
            .context("Plugin download interrupted or timed out; check network/proxy and retry")?;
        if n == 0 {
            break;
        }
        ensure!(bytes.len().saturating_add(n) <= limit, "Plugin download exceeds size limit");
        bytes.extend_from_slice(&chunk[..n]);
        if bytes.len() - reported >= 256 * 1024 {
            reported = bytes.len();
            progress.progress(format!(
                "Downloading plugin: {} KiB{}",
                bytes.len() / 1024,
                total.map(|n| format!(" / {} KiB", n / 1024)).unwrap_or_default()
            ))?;
        }
    }
    progress.check()?;
    Ok(bytes)
}
fn extract(bytes: &[u8], directory: &Path, expected_target: &str) -> Result<()> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
    let bin_names = [format!("bin/{}", executable("chatgpt")), format!("bin/{}", executable("claude"))];
    let names: Vec<&str> = bin_names
        .iter()
        .map(String::as_str)
        .chain(["BUILD.json", "LICENSE", "README.txt", "THIRD_PARTY_LICENSES.txt"])
        .collect();
    ensure!(zip.len() == names.len(), "Unexpected plugin archive members");
    let mut seen = std::collections::HashSet::new();
    for index in 0..zip.len() {
        let mut file = zip.by_index(index)?;
        let name = file.name()?.into_owned();
        ensure!(names.contains(&name.as_str()) && seen.insert(name.clone()), "Unexpected or duplicate plugin path");
        ensure!(
            file.size() <= LIMIT as u64
                && !file.is_dir()
                && file.unix_mode().is_none_or(|mode| mode & 0o170000 != 0o120000),
            "Unsafe plugin member"
        );
        let path = directory.join(&name);
        fs::create_dir_all(path.parent().unwrap())?;
        let mut output = fs::OpenOptions::new().write(true).create_new(true).open(&path)?;
        let n = std::io::copy(&mut file.by_ref().take(LIMIT as u64 + 1), &mut output)?;
        ensure!(n <= LIMIT as u64, "Expanded plugin exceeds limit");
        output.flush()?;
        output.sync_all()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                &path,
                fs::Permissions::from_mode(if bin_names.contains(&name) { 0o700 } else { 0o600 }),
            )?;
        }
    }
    let build: Value = serde_json::from_slice(&state::read_bounded(&directory.join("BUILD.json"), 8192)?)?;
    ensure!(
        build["format_version"] == 1 && build["kind"] == "optional-connections" && build["target"] == expected_target,
        "Plugin package target/format mismatch"
    );
    Ok(())
}
fn install(paths: &AppPaths, id: &str, progress: super::installation::Context) -> Result<()> {
    valid(id)?;
    let target = target()?;
    progress.progress("Checking the latest official plugin release… /cancel cancels")?;
    let release: Value = serde_json::from_slice(&download(
        "https://api.github.com/repos/qinains/i/releases/latest",
        2_000_000,
        &progress,
    )?)?;
    let tag = release["tag_name"].as_str().context("Missing release version")?;
    ensure!(
        tag.starts_with('v') && tag.len() < 40 && tag.bytes().all(|b| b.is_ascii_alphanumeric() || b".-".contains(&b)),
        "Invalid release version"
    );
    let name = format!("i-connections-{target}.zip");
    let asset = release["assets"]
        .as_array()
        .context("Missing release assets")?
        .iter()
        .find(|asset| asset["name"] == name)
        .context("This release has no connection package for this platform")?;
    let digest = asset["digest"]
        .as_str()
        .and_then(|v| v.strip_prefix("sha256:"))
        .context("Release has no verified SHA256 digest")?;
    ensure!(digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit()), "Invalid release digest");
    progress.progress(format!("Downloading {name} ({tag})… /cancel cancels"))?;
    let bytes = download(&format!("https://github.com/qinains/i/releases/download/{tag}/{name}"), LIMIT, &progress)?;
    progress.progress("Verifying SHA-256 and package contents…")?;
    ensure!(hash(&bytes) == digest, "Plugin download checksum mismatch; nothing installed");
    install_bytes(paths, id, &bytes, tag, progress)
}
fn install_local(paths: &AppPaths, id: &str, file: &Path, progress: super::installation::Context) -> Result<()> {
    valid(id)?;
    progress.progress("Checking local plugin package (not a verified official release)…")?;
    let bytes = state::read_bounded(file, LIMIT)?;
    install_bytes(paths, id, &bytes, "local-build", progress)
}
fn install_bytes(
    paths: &AppPaths,
    id: &str,
    bytes: &[u8],
    tag: &str,
    progress: super::installation::Context,
) -> Result<()> {
    let target = target()?;
    fs::create_dir_all(&paths.data_dir)?;
    let packages = state::inside(&paths.data_dir, Path::new("connections/packages"))?;
    fs::create_dir_all(&packages)?;
    let stage = tempfile::tempdir_in(&packages)?;
    extract(bytes, stage.path(), target)?;
    progress.check()?;
    let executable_hash = hash(&state::read_bounded(&stage.path().join("bin").join(executable(id)), LIMIT)?);
    // Unique publication avoids replacing code another running session is using.
    let name = uuid::Uuid::new_v4().to_string();
    progress.progress("Saving the verified plugin (disabled until you enable it)…")?;
    fs::create_dir_all(&paths.config_dir)?;
    let _lock = state::Lock::acquire(&paths.config_dir.join("native-plugins.lock"))?;
    progress.commit(|| {
        let mut value = selection(paths)?;
        fs::rename(stage.path(), packages.join(&name))?;
        value[id] = json!({"enabled":false,"release":tag,"sha256":executable_hash,
            "executable":format!("connections/packages/{name}/bin/{}", executable(id))});
        state::atomic(&paths.config_dir.join("native-plugins.json"), &value)
    })
}
fn helper(paths: &AppPaths, backend: &[String], id: &str, command: &str) -> Result<Value> {
    helper_args(paths, backend, id, &[command])
}
fn helper_args(paths: &AppPaths, backend: &[String], id: &str, arguments: &[&str]) -> Result<Value> {
    let exe = checked_exe(paths, id, true)?;
    let data = credential_directory(paths, backend, id)?;
    let mut command = Command::new(exe);
    super::proxy::snapshot(id)?.apply(&mut command);
    let output = command
        .args(["--data-dir"])
        .arg(data)
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .context("Cannot start login plugin")?;
    ensure!(output.stdout.len() <= 2_000_000 && output.stderr.len() <= 16_000, "Login plugin output exceeds limit");
    if !output.status.success() {
        // Helper diagnostics are deliberately credential-free; never print stdout/account details.
        let message: String = String::from_utf8_lossy(&output.stderr)
            .chars()
            .filter(|c| !c.is_control() || *c == '\n')
            .take(2000)
            .collect();
        bail!("{}: {}", title(id), message.trim());
    }
    serde_json::from_slice(&output.stdout).context("Invalid login plugin result")
}
fn credential_directory(paths: &AppPaths, backend: &[String], id: &str) -> Result<PathBuf> {
    let config = option(backend, "--config").map(PathBuf::from).unwrap_or_else(|| paths.config_dir.join("config.json"));
    if config.try_exists()? {
        let config: Value = serde_json::from_slice(&state::read_bounded(&config, 1_000_000)?)?;
        if config["provider"]["managed_connection"] == id {
            let dir =
                Path::new(config["provider"]["exec"][2].as_str().context("Invalid managed credential directory")?);
            let root = dunce::canonicalize(&paths.data_dir)?;
            let dir = dunce::canonicalize(dir).context("Cannot open saved managed credential directory")?;
            return state::inside(&root, dir.strip_prefix(&root).context("Credential directory escapes plugin data")?);
        }
    }
    state::inside(&paths.data_dir, Path::new(&format!("connections/accounts/{id}")))
}
fn stage_registration(paths: &AppPaths, backend: &[String], id: &str, account: &str, directory: &Path) -> Result<()> {
    ensure!(
        id == "chatgpt",
        "Explicit reauthorization accounts are supported for ChatGPT; use /login claude for its Console profile"
    );
    let source = credential_directory(paths, backend, id)?.join("accounts.json");
    let store: Value = serde_json::from_slice(&state::read_bounded(&source, 2_000_000)?)?;
    let mut selected = store["accounts"]
        .as_array()
        .context("Invalid saved ChatGPT registration")?
        .iter()
        .find(|entry| entry["id"] == account)
        .cloned()
        .context("Unknown saved ChatGPT account; list /plugins status chatgpt")?;
    // Reauthorize the registration, never clone a rotating session into a second lock domain.
    selected["credentials"] = Value::Null;
    state::atomic(
        &directory.join("accounts.json"),
        &json!({"host":store["host"],"active":account,"accounts":[selected]}),
    )
}
fn stage_new_registration(paths: &AppPaths, backend: &[String], directory: &Path) -> Result<()> {
    let base = state::inside(&paths.data_dir, Path::new("connections/accounts/chatgpt"))?;
    fs::create_dir_all(&base)?;
    let _guard = state::Lock::acquire(&base.join("host.lock"))?;
    let file = base.join("host.json");
    let host = if file.try_exists()? {
        let value: Value = serde_json::from_slice(&state::read_bounded(&file, 8192)?)?;
        value["host"].as_str().context("Invalid ChatGPT host identity")?.to_owned()
    } else {
        let source = credential_directory(paths, backend, "chatgpt")?.join("accounts.json");
        let host = if source.try_exists()? {
            let value: Value = serde_json::from_slice(&state::read_bounded(&source, 2_000_000)?)?;
            value["host"].as_str().context("Invalid saved ChatGPT host identity")?.to_owned()
        } else {
            format!("urn:uuid:{}", uuid::Uuid::new_v4())
        };
        state::atomic(&file, &json!({"host":host}))?;
        host
    };
    uuid::Uuid::parse_str(host.strip_prefix("urn:uuid:").context("Invalid ChatGPT host identity")?)?;
    state::atomic(&directory.join("accounts.json"), &json!({"host":host,"active":null,"accounts":[]}))
}
fn activate(
    paths: &AppPaths,
    backend: &mut Vec<String>,
    id: &str,
    model: &str,
    directory: Option<&Path>,
) -> Result<()> {
    ensure!(
        !model.trim().is_empty() && model.len() <= 256 && !model.chars().any(char::is_control),
        "Invalid model name"
    );
    let exe = checked_exe(paths, id, true)?;
    let file = option(backend, "--config").map(PathBuf::from).unwrap_or_else(|| paths.config_dir.join("config.json"));
    fs::create_dir_all(file.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new(".")))?;
    ensure!(!file.is_symlink(), "Configuration cannot be a symlink");
    let _guard = state::Lock::acquire(&file.with_extension("lock"))?;
    let mut value: Value =
        if file.try_exists()? { serde_json::from_slice(&state::read_bounded(&file, 1_000_000)?)? } else { json!({}) };
    ensure!(value.is_object(), "Invalid model configuration");
    if file.try_exists()? {
        let backup = file.with_extension(format!("{}.backup.json", uuid::Uuid::new_v4()));
        state::atomic(&backup, &value)?;
    }
    let data = directory.map(Path::to_path_buf).map(Ok).unwrap_or_else(|| credential_directory(paths, backend, id))?;
    value["provider"] = json!({"completion":"plugin","managed_connection":id,"exec":[exe,"--data-dir",data,"worker"]});
    value["protocol"] = json!(if id == "claude" { "anthropic" } else { "responses" });
    value["base_url"] =
        json!(if id == "claude" { "https://api.anthropic.com/v1" } else { "https://api.openai.com/v1" });
    value["model"] = json!(model);
    value["provider_operations"] = json!(false);
    for key in ["api_key", "api_key_env"] {
        value.as_object_mut().unwrap().remove(key);
    }
    if value.get("max_tokens").is_none() {
        value["max_tokens"] = json!(65536);
    }
    state::atomic(&file, &value)?;
    // Explicit invocation arguments must not override the newly selected connection.
    for flag in ["--config", "--model", "-m", "--base-url", "--protocol", "--api-key", "--api-key-env"] {
        *backend = super::console::without(backend, flag);
    }
    backend.extend(["--config".into(), file.to_string_lossy().into_owned()]);
    Ok(())
}

pub(crate) fn check_connection(paths: &AppPaths, provider: &Value) -> Result<()> {
    if let Some(id) = provider.get("managed_connection") {
        let id = id.as_str().context("Invalid managed connection")?;
        let exe = checked_exe(paths, id, true)?;
        ensure!(
            provider["exec"][0].as_str().map(Path::new) == Some(exe.as_path()),
            "Installed plugin changed; run /login again to reconnect"
        );
    }
    Ok(())
}

pub(crate) fn run_cli(command: &str, args: &[String]) -> Result<i32> {
    use std::io::BufRead;
    let paths = AppPaths::desktop()?;
    if command == "plugins" && args.first().map(String::as_str) == Some("serve") {
        let id = args.get(1).context("Usage: i plugins serve chatgpt|claude [--origin URL] [--port PORT]")?;
        let rest = &args[2..];
        let mut index = 0;
        while index < rest.len() {
            ensure!(
                matches!(rest[index].as_str(), "--origin" | "--port") && index + 1 < rest.len(),
                "Usage: i plugins serve NAME [--origin URL] [--port PORT]"
            );
            index += 2;
        }
        let exe = checked_exe(&paths, id, true)?;
        let data = credential_directory(&paths, &[], id)?;
        let mut command = Command::new(exe);
        super::proxy::snapshot(id)?.apply(&mut command);
        let status = command.arg("--data-dir").arg(data).arg("serve").args(rest).status()?;
        ensure!(status.success(), "Local connection service stopped with an error");
        return Ok(0);
    }
    let mut menu = Menu::default();
    let local = if command == "plugins" && args.iter().any(|arg| arg == "--package") {
        ensure!(
            args.len() == 4 && args[0] == "install" && args[2] == "--package",
            "Usage: i plugins install NAME --package FILE.zip"
        );
        valid(&args[1])?;
        Some((args[1].clone(), dunce::canonicalize(&args[3]).context("Local plugin package not found")?))
    } else {
        None
    };
    let mut backend = Vec::new();
    let line = if command == "plugins" && (args.is_empty() || args == ["list"]) {
        "/plugins".to_owned()
    } else {
        format!("/{command} {}", args.join(" "))
    };
    if args.iter().any(|arg| arg == "--help") {
        println!(
            "i plugins [list|install|enable|disable|update|remove|status|models|logout NAME]\ni plugins serve NAME [--origin URL] [--port PORT]\ni login [chatgpt|claude]\nNative helpers are installed only after permission confirmation. Claude uses Console API billing, not Pro/Max."
        );
        return Ok(0);
    }
    let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let signal = cancelled.clone();
    ctrlc::set_handler(move || signal.store(true, std::sync::atomic::Ordering::Release))?;
    if let Some((id, file)) = local {
        println!(
            "Install {} from local package {}? This is NOT a verified official release. Native code runs with your user permissions. [y/N]",
            title(&id),
            file.display()
        );
        menu.pending = Some(Pending::LocalConfirm(id, file));
    } else if let Some(message) = menu.handle(&line, &paths, &mut backend, false)? {
        println!("{message}");
    }
    if command == "plugins" && (args.is_empty() || args == ["list"]) {
        return Ok(0);
    }
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        loop {
            let mut line = String::new();
            match std::io::stdin().lock().take(8193).read_line(&mut line) {
                Ok(0) | Err(_) => {
                    let _ = tx.send(None);
                    break;
                }
                Ok(_) => {
                    if tx.send(Some(line)).is_err() {
                        break;
                    }
                }
            }
        }
    });
    let mut queued = std::collections::VecDeque::new();
    let mut eof = false;
    while menu.pending.is_some() || menu.working() {
        if cancelled.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(130);
        }
        if let Some(message) = menu.poll(&paths)? {
            println!("{message}");
        }
        if menu.pending.is_none() && !menu.working() {
            break;
        }
        let next = if !menu.working() && !queued.is_empty() {
            queued.pop_front()
        } else {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(Some(line)) => Some(line),
                Ok(None) => {
                    eof = true;
                    None
                }
                Err(_) => None,
            }
        };
        if let Some(line) = next {
            ensure!(line.len() <= 8192, "Menu input is too long");
            if menu.working() && !line.starts_with('/') {
                queued.push_back(line);
                continue;
            }
            if let Some(message) = menu.handle(line.trim(), &paths, &mut backend, false)? {
                println!("{message}");
            }
        } else if eof && queued.is_empty() && !menu.working() {
            println!("[plugin operation cancelled; current connection preserved]");
            break;
        }
        if eof && menu.working() {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    Ok(0)
}

enum Pending {
    SwitchModel(Vec<String>),
    LocalConfirm(String, PathBuf),
    Plugin,
    Login,
    Confirm(String),
    Remove(String),
    Model(String, Vec<String>, tempfile::TempDir),
}
fn model_ids(value: &Value) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    value
        .get("data")
        .or_else(|| value.get("models"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|entry| entry["visibility"] != "hide")
        .filter_map(|entry| entry["id"].as_str().or_else(|| entry["slug"].as_str()))
        .filter(|id| !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_control))
        .filter(|id| seen.insert((*id).to_owned()))
        .take(100)
        .map(str::to_owned)
        .collect()
}
#[derive(Default)]
pub(crate) struct Menu {
    pending: Option<Pending>,
    job: Option<super::connection_job::Job>,
    installation: Option<super::installation::Job>,
}
impl Menu {
    fn working(&self) -> bool {
        self.job.is_some() || self.installation.is_some()
    }
    pub(crate) fn poll(&mut self, paths: &AppPaths) -> Result<Option<String>> {
        if let Some(installation) = &self.installation {
            return match installation.poll() {
                Some(super::installation::Event::Progress(message)) => Ok(Some(message)),
                Some(super::installation::Event::Finished(result)) => {
                    let id = self.installation.take().unwrap().id.clone();
                    result?;
                    Ok(Some(format!("Installed {} (disabled). Enable with /plugins enable {id}", title(&id))))
                }
                None => Ok(None),
            };
        }
        let Some(job) = self.job.as_mut() else { return Ok(None) };
        let result = job.poll();
        let value = match result {
            Ok(Some(value)) => value,
            Ok(None) => return Ok(None),
            Err(error) => {
                if self.job.as_ref().is_some_and(|job| job.phase == "models" && job.directory.is_some()) {
                    let mut job = self.job.take().unwrap();
                    self.pending = Some(Pending::Model(job.id.clone(), vec![], job.directory.take().unwrap()));
                    return Ok(Some(format!(
                        "Model catalog unavailable: {error:#}. Select a model by entering its ID manually, or /cancel to preserve your current connection."
                    )));
                }
                self.job = None;
                return Err(error);
            }
        };
        let mut job = self.job.take().unwrap();
        if job.phase == "models" && job.directory.is_none() {
            let models = model_ids(&value);
            let list =
                models.iter().enumerate().map(|(n, id)| format!("{}  {id}", n + 1)).collect::<Vec<_>>().join("\n");
            self.pending = Some(Pending::SwitchModel(models));
            return Ok(Some(format!("Choose a model number or type its ID; /cancel preserves settings.\n{list}")));
        }
        let directory = job.directory.take().unwrap();
        if job.phase == "login" {
            ensure!(
                value["signed_in"] == true,
                "Plugin did not confirm successful login; current connection preserved"
            );
            self.job = Some(super::connection_job::Job::start(paths, job.id.clone(), "models", directory, None)?);
            return Ok(Some("Signed in; loading available models… /cancel cancels.".into()));
        }
        let models = model_ids(&value);
        let list = models.iter().enumerate().map(|(n, id)| format!("{}  {id}", n + 1)).collect::<Vec<_>>().join("\n");
        self.pending = Some(Pending::Model(job.id.clone(), models, directory));
        Ok(Some(format!(
            "Signed in. Select a model number or enter its ID (no model request is sent):\n{list}\n/cancel preserves your current connection."
        )))
    }
    pub(crate) fn handle(
        &mut self,
        line: &str,
        paths: &AppPaths,
        backend: &mut Vec<String>,
        busy: bool,
    ) -> Result<Option<String>> {
        if let Some(message) = super::proxy::handle(line, paths)? {
            return Ok(Some(message));
        }
        let args: Vec<_> = line.split_whitespace().collect();
        if self.installation.is_some() {
            match line.trim() {
                "/cancel" => {
                    let job = self.installation.take().unwrap();
                    let cancelled = job.cancel();
                    return Ok(Some(if cancelled {
                        "[plugin installation cancelled; no late download will be installed]".into()
                    } else {
                        format!("Installed {} (disabled); installation already completed.", title(&job.id))
                    }));
                }
                "/exit" => {
                    self.installation = None;
                    return Ok(None);
                }
                "/help" => return Ok(None),
                _ => return Ok(Some("[plugin installation in progress; /cancel cancels]".into())),
            }
        }
        if self.job.is_some() {
            match line.trim() {
                "/cancel" => {
                    self.job = None;
                    return Ok(Some("[login cancelled; pending local credentials discarded; current connection preserved. Remote app registration may remain in provider settings.]".into()));
                }
                "/exit" => {
                    self.job = None;
                    return Ok(None);
                }
                "/help" => return Ok(None),
                _ => return Ok(Some("[waiting for provider login/model directory; /cancel cancels]".into())),
            }
        }
        let own = matches!(args.first().copied(), Some("/plugins" | "/login" | "/cancel" | "/model"));
        if !own && (self.pending.is_none() || line.starts_with('/')) {
            self.pending = None;
            return Ok(None);
        }
        if busy {
            self.pending = None;
            return Ok(Some("[finish or stop the current task before managing plugins/connections]".into()));
        }
        if line == "/cancel" {
            self.pending = None;
            return Ok(Some("[plugin operation cancelled]".into()));
        }
        if own {
            self.pending = None;
        }
        if args.first() == Some(&"/model") {
            if matches!(args.as_slice(), [_] | [_, "list"]) {
                let status = super::model_settings::status(paths, backend)?;
                let config = super::model_settings::current(paths, backend)?;
                if let Some(id) = config["provider"]["managed_connection"].as_str().filter(|id| IDS.contains(id)) {
                    check_connection(paths, &config["provider"])?;
                    let data = PathBuf::from(
                        config["provider"]["exec"][2].as_str().context("Invalid managed credential directory")?,
                    );
                    self.job = Some(super::connection_job::Job::catalog(paths, id.to_owned(), data)?);
                    return Ok(Some(format!("{status}\nLoading provider model catalog… /cancel cancels.")));
                }
                return Ok(Some(format!(
                    "{status}\nEnter /model MODEL to switch. This API connection has no native catalog plugin."
                )));
            }
            return super::model_settings::command(&args, paths, backend).map(Some);
        }
        if let Some(Pending::Model(_, models, _)) = &self.pending {
            if !models.is_empty() && line.trim().parse::<usize>().is_ok_and(|n| n == 0 || n > models.len()) {
                return Ok(Some(
                    "[model number out of range; select a listed number, enter a model ID, or /cancel]".into(),
                ));
            }
            if line.trim().is_empty() || line.len() > 256 || line.chars().any(char::is_control) {
                return Ok(Some("[invalid model ID; try again or /cancel]".into()));
            }
        }
        let selected = match self.pending.take() {
            Some(Pending::Remove(id)) => {
                if !matches!(line.trim(), "y" | "Y" | "yes") {
                    return Ok(Some("[plugin removal cancelled]".into()));
                }
                save_selection(paths, |v| {
                    v.as_object_mut().unwrap().remove(&id);
                    Ok(())
                })?;
                return Ok(Some(format!(
                    "Removed {} from enabled/installed selection. Saved credentials and old package files were retained; use i plugins logout {id} before removal to revoke a session.",
                    title(&id)
                )));
            }
            Some(Pending::SwitchModel(models)) => {
                let input = line.trim();
                let model = if let Ok(n) = input.parse::<usize>() {
                    if let Some(model) = n.checked_sub(1).and_then(|n| models.get(n)) {
                        model.clone()
                    } else {
                        self.pending = Some(Pending::SwitchModel(models));
                        return Ok(Some("[model number out of range; try again or /cancel]".into()));
                    }
                } else {
                    input.to_owned()
                };
                return super::model_settings::change(paths, backend, Some(&model), None).map(Some);
            }
            Some(Pending::LocalConfirm(id, file)) => {
                if !matches!(line.trim(), "y" | "Y" | "yes") {
                    return Ok(Some("[plugin installation cancelled]".into()));
                }
                let paths = paths.clone();
                let plugin = id.clone();
                self.installation = Some(super::installation::Job::start(id, move |progress| {
                    install_local(&paths, &plugin, &file, progress)
                })?);
                return Ok(Some("Checking local plugin package… /cancel cancels.".into()));
            }
            Some(Pending::Confirm(id)) => {
                if !matches!(line.trim(), "y" | "Y" | "yes") {
                    return Ok(Some("[plugin installation cancelled]".into()));
                }
                let paths = paths.clone();
                let plugin = id.clone();
                self.installation =
                    Some(super::installation::Job::start(id, move |progress| install(&paths, &plugin, progress))?);
                return Ok(Some("Starting plugin installation… /cancel cancels. Network connection timeout: 10 seconds; each download: at most 120 seconds.".into()));
            }
            Some(Pending::Model(id, models, directory)) => {
                let model = line
                    .trim()
                    .parse::<usize>()
                    .ok()
                    .and_then(|n| n.checked_sub(1))
                    .and_then(|n| models.get(n))
                    .map(String::as_str)
                    .unwrap_or(line.trim());
                activate(paths, backend, &id, model, Some(directory.path()))?;
                let _ = directory.keep();
                return Ok(Some(format!(
                    "Connected {} / {model}. Configuration saved; ready for your next task.",
                    title(&id)
                )));
            }
            Some(Pending::Plugin) => Some((false, line.trim().to_owned())),
            Some(Pending::Login) => Some((true, line.trim().to_owned())),
            None => None,
        };
        if line == "/plugins" || line == "/plugins list" {
            let values = selection(paths)?;
            self.pending = Some(Pending::Plugin);
            let mut text = String::from("Native login plugins (optional; separate from workspace WASM tools):\n");
            for (n, id) in IDS.iter().enumerate() {
                text.push_str(&format!(
                    "{}  {}  [{}]\n",
                    n + 1,
                    title(id),
                    if values[*id]["enabled"] == true {
                        "enabled"
                    } else if values.get(*id).is_some() {
                        "installed, disabled"
                    } else {
                        "not installed"
                    }
                ));
            }
            text.push_str("Choose 1/2, or /plugins install chatgpt | /plugins enable chatgpt | /plugins disable chatgpt.\nWASM tool plugins: https://i.lninl.com/app/ -> Plugins. /cancel cancels.");
            return Ok(Some(text));
        }
        let mut login_id = None;
        let mut reauthorize = None;
        if let Some((login, choice)) = selected {
            let id = match choice.as_str() {
                "1" => "chatgpt",
                "2" => "claude",
                other => other,
            };
            valid(id)?;
            if login {
                login_id = Some(id.to_owned());
            } else if selection(paths)?.get(id).is_some() {
                enable(paths, id, true)?;
                return Ok(Some(format!("Enabled {}. Use /login {id}", title(id))));
            } else {
                self.pending = Some(Pending::Confirm(id.into()));
                return Ok(Some(format!(
                    "Install {}? Downloads official native code running with your user permissions; credentials stay outside workspaces/WASM plugins. [y/N]",
                    title(id)
                )));
            }
        } else if args.first() == Some(&"/plugins") {
            match args.as_slice() {
                [_, "install" | "update", id] => {
                    valid(id)?;
                    self.pending = Some(Pending::Confirm((*id).into()));
                    return Ok(Some(format!(
                        "Install {}? Downloads official native code running with your user permissions; credentials stay outside workspaces/WASM plugins. [y/N]",
                        title(id)
                    )));
                }
                [_, "enable", id] | [_, "disable", id] => {
                    valid(id)?;
                    enable(paths, id, args[1] == "enable")?;
                    return Ok(Some(format!("{} {}", title(id), args[1])));
                }
                [_, "reconnect", id] => {
                    valid(id)?;
                    let config = super::model_settings::current(paths, backend)?;
                    ensure!(
                        config["provider"]["managed_connection"] == *id,
                        "Reconnect only updates the currently selected managed connection; use /login to select another account"
                    );
                    let model = config["model"].as_str().context("Current connection has no model")?;
                    activate(paths, backend, id, model, None)?;
                    return Ok(Some(format!(
                        "Reconnected {} to the verified installed helper; saved account, model and settings preserved. No login or model request was sent.",
                        title(id)
                    )));
                }
                [_, "remove", id] => {
                    valid(id)?;
                    self.pending = Some(Pending::Remove((*id).into()));
                    return Ok(Some(format!(
                        "Remove {}? Managed connections will stop working. Credentials and old package files are retained; run /plugins logout {id} first to revoke your session. [y/N]",
                        title(id)
                    )));
                }
                [_, action @ ("status" | "models" | "logout"), id] => {
                    valid(id)?;
                    return Ok(Some(serde_json::to_string_pretty(&helper(paths, backend, id, action)?)?));
                }
                [_, "use", id, account] => {
                    valid(id)?;
                    ensure!(
                        !account.is_empty() && account.len() <= 256 && !account.chars().any(char::is_control),
                        "Invalid account ID"
                    );
                    return Ok(Some(serde_json::to_string_pretty(&helper_args(
                        paths,
                        backend,
                        id,
                        &["use", account],
                    )?)?));
                }
                [_, "logout", id, "--account", account] => {
                    valid(id)?;
                    return Ok(Some(serde_json::to_string_pretty(&helper_args(
                        paths,
                        backend,
                        id,
                        &["logout", "--account", account],
                    )?)?));
                }
                _ => bail!(
                    "Usage: /plugins [list|install|enable|disable|update|remove|status|models|logout chatgpt|claude]"
                ),
            }
        } else if args.first() == Some(&"/login") {
            match args.as_slice() {
                [_] => {
                    let values = selection(paths)?;
                    let entries: Vec<_> = IDS
                        .iter()
                        .enumerate()
                        .filter(|(_, id)| values[**id]["enabled"] == true)
                        .map(|(n, id)| format!("{}  {}", n + 1, title(id)))
                        .collect();
                    if entries.is_empty() {
                        return Ok(Some("No enabled login plugins. Use /plugins to install and enable ChatGPT or Claude Console API.".into()));
                    }
                    self.pending = Some(Pending::Login);
                    return Ok(Some(format!(
                        "Choose a login plugin (1/2 or name):\n{}\n/cancel cancels.",
                        entries.join("\n")
                    )));
                }
                [_, id] => {
                    valid(id)?;
                    login_id = Some((*id).into());
                }
                [_, id, "--account", account] => {
                    valid(id)?;
                    ensure!(
                        !account.is_empty() && account.len() <= 256 && !account.chars().any(char::is_control),
                        "Invalid account ID"
                    );
                    login_id = Some((*id).into());
                    reauthorize = Some(*account);
                }
                _ => bail!("Usage: /login [chatgpt|claude] [--account ID]"),
            }
        }
        if let Some(id) = login_id {
            checked_exe(paths, &id, true)?;
            let base = state::inside(&paths.data_dir, Path::new(&format!("connections/accounts/{id}")))?;
            fs::create_dir_all(&base)?;
            let directory = tempfile::tempdir_in(base)?;
            if let Some(account) = reauthorize {
                stage_registration(paths, backend, &id, account, directory.path())?;
            } else if id == "chatgpt" {
                stage_new_registration(paths, backend, directory.path())?;
            }
            self.job = Some(super::connection_job::Job::start(paths, id, "login", directory, reauthorize)?);
            return Ok(Some(format!(
                "Waiting for {} login in your system browser… /cancel cancels.",
                title(self.job.as_ref().unwrap().id.as_str())
            )));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_catalogs_support_native_and_api_shapes_without_hidden_duplicates() {
        assert_eq!(model_ids(&json!({"data":[{"id":"api-model"}]})), vec!["api-model"]);
        assert_eq!(
            model_ids(
                &json!({"models":[{"slug":"native-model"},{"slug":"native-model"},{"slug":"hidden","visibility":"hide"}]})
            ),
            vec!["native-model"]
        );
    }
    #[test]
    fn installer_uses_the_tls_backend_enabled_in_the_native_binary() {
        assert_eq!(
            download_config(&super::super::proxy::Setting::Inherit).unwrap().tls_config().provider(),
            ureq::tls::TlsProvider::NativeTls
        );
    }
    #[test]
    fn menu_keeps_rendering_progress_and_cancel_does_not_wait_for_network() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let (entered, waiting) = std::sync::mpsc::channel();
        let (release, gate) = std::sync::mpsc::channel();
        let mut menu = Menu {
            installation: Some(
                super::super::installation::Job::start("chatgpt".into(), move |progress| {
                    progress.progress("Downloading test package: 256 KiB")?;
                    entered.send(()).unwrap();
                    gate.recv().unwrap();
                    progress.check()
                })
                .unwrap(),
            ),
            ..Menu::default()
        };
        waiting.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(menu.poll(&paths).unwrap().unwrap().contains("256 KiB"));
        assert!(menu.handle("/cancel", &paths, &mut vec![], false).unwrap().unwrap().contains("cancelled"));
        assert!(!menu.working());
        assert!(!paths.config_dir.join("native-plugins.json").exists());
        release.send(()).unwrap();
    }
    fn paths(root: &Path) -> AppPaths {
        let paths = AppPaths {
            config_dir: root.join("config"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            runtime_dir: root.join("runtime"),
        };
        for dir in [&paths.config_dir, &paths.data_dir] {
            fs::create_dir_all(dir).unwrap();
        }
        paths
    }
    fn registered(paths: &AppPaths) -> PathBuf {
        let exe = paths.data_dir.join("fake-helper");
        fs::write(&exe, b"fixture executable").unwrap();
        save_selection(paths, |v| {
            v["chatgpt"] = json!({"enabled":true,"executable":"fake-helper","sha256":hash(b"fixture executable")});
            Ok(())
        })
        .unwrap();
        exe
    }
    #[test]
    fn activation_preserves_advanced_settings_and_backs_up_without_leaking_api_key() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let exe = registered(&paths);
        let file = paths.config_dir.join("config.json");
        state::atomic(&file,&json!({"plugins":"builtin.json","io_limit_bytes":1000,"max_tokens":1234,"api_key":"YOUR_API_KEY","provider_operations":true})).unwrap();
        let mut args = vec!["--model".into(), "old".into(), "--protocol=openai".into(), "-C".into(), ".".into()];
        activate(&paths, &mut args, "chatgpt", "test-model", None).unwrap();
        let value: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        assert_eq!(value["model"], "test-model");
        assert_eq!(value["provider"]["exec"][0], json!(dunce::canonicalize(exe).unwrap()));
        assert_eq!(value["plugins"], "builtin.json");
        assert_eq!(value["max_tokens"], 1234);
        assert_eq!(value["provider_operations"], false);
        assert!(!value.to_string().contains("YOUR_API_KEY"));
        assert!(!args.iter().any(|s| s == "old" || s == "--protocol=openai"));
        let backups: Vec<_> = fs::read_dir(&paths.config_dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with("backup.json"))
            .collect();
        assert_eq!(backups.len(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(backups[0].path()).unwrap().permissions().mode() & 0o777, 0o600);
        }
    }
    #[test]
    fn disabled_or_modified_helpers_never_change_connection_configuration() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let exe = registered(&paths);
        enable(&paths, "chatgpt", false).unwrap();
        assert!(activate(&paths, &mut vec![], "chatgpt", "test", None).is_err());
        assert!(!paths.config_dir.join("config.json").exists());
        fs::write(&exe, b"changed").unwrap();
        assert!(enable(&paths, "chatgpt", true).is_err());
        assert!(!selection(&paths).unwrap()["chatgpt"]["enabled"].as_bool().unwrap());
    }
    #[test]
    fn selected_config_reconnect_preserves_its_account_instead_of_default_account() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        registered(&paths);
        let default_account = paths.data_dir.join("account-a");
        let selected_account = paths.data_dir.join("account-b");
        fs::create_dir_all(&default_account).unwrap();
        fs::create_dir_all(&selected_account).unwrap();
        let selected_account = dunce::canonicalize(selected_account).unwrap();
        let config = |account: &Path| json!({"model":"current-model","provider":{"completion":"plugin","managed_connection":"chatgpt","exec":["retired-helper","--data-dir",account,"worker"]}});
        let default = config(&default_account);
        state::atomic(&paths.config_dir.join("config.json"), &default).unwrap();
        let selected = paths.config_dir.join("selected.json");
        state::atomic(&selected, &config(&selected_account)).unwrap();
        let mut backend = vec!["--config".into(), selected.to_string_lossy().into_owned()];
        Menu::default().handle("/plugins reconnect chatgpt", &paths, &mut backend, false).unwrap();
        let after = super::super::model_settings::current(&paths, &backend).unwrap();
        assert_eq!(after["provider"]["exec"][2], json!(selected_account));
        assert_eq!(super::super::model_settings::current(&paths, &[]).unwrap(), default);
    }
    #[cfg(unix)]
    #[test]
    fn selected_config_account_commands_use_its_directory_not_the_default() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let exe = registered(&paths);
        let script = b"#!/bin/sh\nprintf '{\"directory\":\"%s\",\"action\":\"%s\"}\\n' \"$2\" \"$3\"\n";
        fs::write(&exe, script).unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o700)).unwrap();
        save_selection(&paths, |value| {
            value["chatgpt"]["sha256"] = json!(hash(script));
            Ok(())
        })
        .unwrap();
        let default_account = paths.data_dir.join("account-a");
        let selected_account = paths.data_dir.join("account-b");
        fs::create_dir_all(&default_account).unwrap();
        fs::create_dir_all(&selected_account).unwrap();
        let selected_account = dunce::canonicalize(selected_account).unwrap();
        let config = |account: &Path| json!({"provider":{"managed_connection":"chatgpt","exec":[exe,"--data-dir",account,"worker"]}});
        let default = config(&default_account);
        state::atomic(&paths.config_dir.join("config.json"), &default).unwrap();
        let selected = paths.config_dir.join("selected.json");
        state::atomic(&selected, &config(&selected_account)).unwrap();
        let mut backend = vec!["--config".into(), selected.to_string_lossy().into_owned()];
        for command in [
            "status chatgpt",
            "models chatgpt",
            "use chatgpt saved",
            "logout chatgpt",
            "logout chatgpt --account saved",
        ] {
            let output =
                Menu::default().handle(&format!("/plugins {command}"), &paths, &mut backend, false).unwrap().unwrap();
            let value: Value = serde_json::from_str(&output).unwrap();
            assert_eq!(value["directory"], json!(selected_account), "{command}");
        }
        assert_eq!(super::super::model_settings::current(&paths, &[]).unwrap(), default);
    }
    #[test]
    fn selected_config_registration_uses_its_account_and_keeps_one_stable_host() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let account_a = paths.data_dir.join("account-a");
        let account_b = paths.data_dir.join("account-b");
        let host_b = format!("urn:uuid:{}", uuid::Uuid::new_v4());
        for (account, host) in
            [(&account_a, format!("urn:uuid:{}", uuid::Uuid::new_v4())), (&account_b, host_b.clone())]
        {
            fs::create_dir_all(account).unwrap();
            state::atomic(&account.join("accounts.json"), &json!({"host":host,"active":"saved","accounts":[{"id":"saved","client_id":account.file_name().unwrap().to_str().unwrap(),"credentials":{"refresh_token":"SYNTHETIC_ONLY"}}]})).unwrap();
        }
        let config = |account: &Path| json!({"provider":{"managed_connection":"chatgpt","exec":["helper","--data-dir",account,"worker"]}});
        state::atomic(&paths.config_dir.join("config.json"), &config(&account_a)).unwrap();
        let selected = paths.config_dir.join("selected.json");
        state::atomic(&selected, &config(&account_b)).unwrap();
        let backend = vec!["--config".into(), selected.to_string_lossy().into_owned()];
        let staged = tempfile::tempdir().unwrap();
        stage_registration(&paths, &backend, "chatgpt", "saved", staged.path()).unwrap();
        let read = |dir: &Path| serde_json::from_slice::<Value>(&fs::read(dir.join("accounts.json")).unwrap()).unwrap();
        let registration = read(staged.path());
        assert_eq!(registration["host"], host_b);
        assert_eq!(registration["accounts"][0]["client_id"], "account-b");
        assert!(registration["accounts"][0]["credentials"].is_null());
        stage_new_registration(&paths, &backend, staged.path()).unwrap();
        assert_eq!(read(staged.path())["host"], host_b);
        stage_new_registration(&paths, &[], staged.path()).unwrap();
        assert_eq!(read(staged.path())["host"], host_b);
        assert_eq!(read(&account_b)["accounts"][0]["credentials"]["refresh_token"], "SYNTHETIC_ONLY");
    }
    #[test]
    fn reconnect_updates_only_the_installed_helper_binding_without_login_or_account_replacement() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let exe = registered(&paths);
        let account = paths.data_dir.join("connections/accounts/previous");
        fs::create_dir_all(&account).unwrap();
        let account = dunce::canonicalize(account).unwrap();
        fs::write(account.join("accounts.json"), b"synthetic-private-existing-state").unwrap();
        let provider = json!({"completion":"plugin","managed_connection":"chatgpt","exec":[paths.data_dir.join("retired-helper"),"--data-dir",account,"worker"]});
        let config = json!({"model":"current-model","provider":provider,"max_tokens":1234,"extra_body":{"reasoning":{"effort":"high"}}});
        state::atomic(&paths.config_dir.join("config.json"), &config).unwrap();
        let result = Menu::default().handle("/plugins reconnect chatgpt", &paths, &mut vec![], false).unwrap().unwrap();
        assert!(result.contains("No login or model request"));
        let after = super::super::model_settings::current(&paths, &[]).unwrap();
        assert_eq!(after["provider"]["exec"][0], json!(dunce::canonicalize(exe).unwrap()));
        assert_eq!(after["provider"]["exec"][2], config["provider"]["exec"][2]);
        assert_eq!(after["model"], config["model"]);
        assert_eq!(after["extra_body"], config["extra_body"]);
        assert_eq!(after["max_tokens"], 1234);
        assert_eq!(fs::read(account.join("accounts.json")).unwrap(), b"synthetic-private-existing-state");
        assert!(Menu::default().handle("/plugins reconnect claude", &paths, &mut vec![], false).is_err());
    }
    fn archive(target: &str, extra: bool) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for name in [
            format!("bin/{}", executable("chatgpt")),
            format!("bin/{}", executable("claude")),
            "BUILD.json".into(),
            "LICENSE".into(),
            "README.txt".into(),
            "THIRD_PARTY_LICENSES.txt".into(),
        ] {
            zip.start_file(&name, zip::write::SimpleFileOptions::default()).unwrap();
            if name == "BUILD.json" {
                write!(zip, "{}", json!({"format_version":1,"kind":"optional-connections","target":target})).unwrap();
            } else {
                zip.write_all(b"fixture").unwrap();
            }
        }
        if extra {
            zip.start_file("../escape", zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(b"bad").unwrap();
        }
        zip.finish().unwrap().into_inner()
    }
    #[test]
    fn local_packages_use_the_same_validation_and_never_enable_automatically() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let file = root.path().join("local.zip");
        fs::write(&file, archive(target().unwrap(), false)).unwrap();
        let p = paths.clone();
        let f = file.clone();
        let job = super::super::installation::Job::start("chatgpt".into(), move |progress| {
            install_local(&p, "chatgpt", &f, progress)
        })
        .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(super::super::installation::Event::Finished(result)) = job.poll() {
                result.unwrap();
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let before = selection(&paths).unwrap();
        assert_eq!(before["chatgpt"]["release"], "local-build");
        assert_eq!(before["chatgpt"]["enabled"], false);
        assert!(checked_exe(&paths, "chatgpt", false).is_ok());
        fs::write(&file, archive("wrong-target", false)).unwrap();
        let p = paths.clone();
        let job = super::super::installation::Job::start("chatgpt".into(), move |progress| {
            install_local(&p, "chatgpt", &file, progress)
        })
        .unwrap();
        loop {
            if let Some(super::super::installation::Event::Finished(result)) = job.poll() {
                assert!(result.is_err());
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(selection(&paths).unwrap(), before);
    }
    #[test]
    fn archives_require_exact_members_and_correct_target() {
        let root = tempfile::tempdir().unwrap();
        assert!(extract(&archive("fixture", false), root.path(), "fixture").is_ok());
        let wrong = tempfile::tempdir().unwrap();
        assert!(extract(&archive("foreign", false), wrong.path(), "fixture").is_err());
        let extra = tempfile::tempdir().unwrap();
        assert!(extract(&archive("fixture", true), extra.path(), "fixture").is_err());
        assert!(!extra.path().parent().unwrap().join("escape").exists());
    }
    #[test]
    fn pending_menus_do_not_consume_other_commands_or_busy_tasks() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let mut menu = Menu::default();
        let mut args = vec![];
        menu.handle("/plugins", &paths, &mut args, false).unwrap();
        assert!(menu.handle("/help", &paths, &mut args, false).unwrap().is_none());
        assert!(menu.handle("ordinary task", &paths, &mut args, false).unwrap().is_none());
        assert!(
            menu.handle("/plugins install chatgpt", &paths, &mut args, true).unwrap().unwrap().contains("current task")
        );
        assert!(menu.pending.is_none());
    }
    #[test]
    fn out_of_range_model_number_keeps_pending_login_and_does_not_write_config() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        registered(&paths);
        let mut menu = Menu {
            pending: Some(Pending::Model(
                "chatgpt".into(),
                vec!["fixture-model".into()],
                tempfile::tempdir_in(&paths.data_dir).unwrap(),
            )),
            ..Menu::default()
        };
        let result = menu.handle("9", &paths, &mut vec![], false).unwrap().unwrap();
        assert!(result.contains("range"), "{result}");
        assert!(menu.pending.is_some());
        assert!(!paths.config_dir.join("config.json").exists());
    }
    #[test]
    fn reauthorization_stages_registration_without_copying_rotating_tokens() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let auth = paths.data_dir.join("connections/accounts/chatgpt");
        fs::create_dir_all(&auth).unwrap();
        let original = json!({"host":format!("urn:uuid:{}",uuid::Uuid::new_v4()),"active":"old","accounts":[{"id":"old","client_id":"issued","credentials":{"access_token":"SYNTHETIC_ACCESS","refresh_token":"SYNTHETIC_REFRESH"}}]});
        state::atomic(&auth.join("accounts.json"), &original).unwrap();
        let stage = tempfile::tempdir().unwrap();
        stage_registration(&paths, &[], "chatgpt", "old", stage.path()).unwrap();
        let copy: Value = serde_json::from_slice(&fs::read(stage.path().join("accounts.json")).unwrap()).unwrap();
        assert_eq!(copy["host"], original["host"]);
        assert_eq!(copy["accounts"][0]["client_id"], "issued");
        assert!(copy["accounts"][0]["credentials"].is_null());
        assert!(!copy.to_string().contains("SYNTHETIC_"));
        assert_eq!(serde_json::from_slice::<Value>(&fs::read(auth.join("accounts.json")).unwrap()).unwrap(), original);
    }
    #[test]
    fn isolated_pending_logins_reuse_the_same_host_identity() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        stage_new_registration(&paths, &[], first.path()).unwrap();
        stage_new_registration(&paths, &[], second.path()).unwrap();
        let read = |dir: &Path| serde_json::from_slice::<Value>(&fs::read(dir.join("accounts.json")).unwrap()).unwrap();
        assert_eq!(read(first.path())["host"], read(second.path())["host"]);
        assert_eq!(read(first.path())["accounts"], json!([]));
    }
}
