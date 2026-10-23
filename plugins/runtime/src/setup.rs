use super::*;
use std::io::IsTerminal;
fn prompt(label: &str, default: &str) -> Result<String> {
    ensure!(io::stdin().is_terminal(), "{label} is required without a TTY; supply setup options");
    eprint!("{label}{}: ", if default.is_empty() { String::new() } else { format!(" [{default}]") });
    io::stderr().flush()?;
    let mut line = String::new();
    io::stdin().lock().take(8193).read_line(&mut line)?;
    ensure!(line.len() <= 8192, "input too long");
    let line = line.trim();
    Ok(if line.is_empty() { default.into() } else { line.into() })
}
fn presets(name: &str) -> Result<(&'static str, &'static str, &'static str)> {
    match name {
        "deepseek" => Ok(("deepseek", "https://api.deepseek.com", "DEEPSEEK_API_KEY")),
        "openai" | "responses" => Ok(("responses", "https://api.openai.com/v1", "OPENAI_API_KEY")),
        "openai-chat" => Ok(("openai", "https://api.openai.com/v1", "OPENAI_API_KEY")),
        "anthropic" => Ok(("anthropic", "https://api.anthropic.com", "ANTHROPIC_API_KEY")),
        _ => {
            bail!("choose deepseek, openai, openai-chat or anthropic; custom adapters remain configurable in API JSON")
        }
    }
}
fn endpoint(value: &str) -> Result<String> {
    let url = url::Url::parse(value)?;
    ensure!(
        matches!(url.scheme(), "http" | "https")
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "endpoint must be an HTTP(S) directory without credentials/query/fragment"
    );
    Ok(url.as_str().trim_end_matches('/').to_owned())
}
fn env_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().enumerate().all(|(i, c)| c == b'_' || c.is_ascii_alphabetic() || i > 0 && c.is_ascii_digit())
}
fn generated_legacy(old: &Value, new: &Value) -> bool {
    if old.as_object().is_none_or(|o| o.keys().any(|k| !matches!(k.as_str(), "tools" | "hooks" | "provider"))) {
        return false;
    }
    for section in ["tools", "hooks", "provider"] {
        let old = if section == "provider" {
            old.get(section).map(|v| vec![v.clone()]).unwrap_or_default()
        } else {
            old[section].as_array().cloned().unwrap_or_default()
        };
        let new = if section == "provider" {
            vec![new[section].clone()]
        } else {
            new[section].as_array().cloned().unwrap_or_default()
        };
        if section == "provider" && old.is_empty() {
            continue;
        }
        if old.len() != new.len() {
            return false;
        }
        for (mut a, mut b) in old.into_iter().zip(new) {
            let module = b["module"].as_str().unwrap_or("");
            let standard = a["exec"].as_array().is_some_and(|v| {
                let name =
                    v.first().and_then(Value::as_str).and_then(|s| Path::new(s).file_stem()).and_then(|s| s.to_str());
                matches!(name, Some("i" | "i-plugins"))
                    && (v.len() == 2 && v[1] == module || v.len() == 3 && v[1] == "--runtime" && v[2] == module)
            });
            if a["cwd"] != b["cwd"]
                || a.get("module").is_some_and(|m| m != &b["module"])
                || !(standard || a["exec"] == b["exec"])
            {
                return false;
            }
            for value in [&mut a, &mut b] {
                let Some(o) = value.as_object_mut() else { return false };
                for key in ["exec", "cwd", "module"] {
                    o.remove(key);
                }
            }
            if a != b {
                return false;
            }
        }
    }
    true
}
fn secret() -> Result<String> {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::Duration;
    let cancelled = Arc::new(AtomicBool::new(false));
    let signal = cancelled.clone();
    let input_active = Arc::new(AtomicBool::new(true));
    let active = input_active.clone();
    ctrlc::set_handler(move || {
        if active.load(Ordering::Acquire) {
            signal.store(true, Ordering::Release);
        } else {
            std::process::exit(130);
        }
    })?;
    let guard = super::super::terminal_mode::Mode::enter()?;
    eprint!("API key (hidden, stored in OS credential storage): ");
    io::stderr().flush()?;
    let (tx, rx) = std::sync::mpsc::sync_channel(64);
    std::thread::spawn(move || {
        let mut input = io::stdin().lock();
        loop {
            let mut byte = [0];
            if input.read_exact(&mut byte).is_err() {
                break;
            }
            if tx.send(byte[0]).is_err() || matches!(byte[0], b'\r' | b'\n' | 3 | 4) {
                break;
            }
        }
    });
    let result = (|| -> Result<String> {
        let mut bytes = Vec::new();
        loop {
            ensure!(!cancelled.load(Ordering::Acquire), "credential entry cancelled");
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(b'\r' | b'\n') => break,
                Ok(3 | 4) => bail!("credential entry cancelled"),
                Ok(8 | 127) => {
                    bytes.pop();
                }
                Ok(b) => {
                    ensure!(bytes.len() < 8192, "API key too long");
                    bytes.push(b);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => (),
                Err(_) => bail!("credential input closed"),
            }
        }
        String::from_utf8(bytes).context("invalid credential encoding")
    })();
    drop(guard);
    input_active.store(false, Ordering::Release);
    eprintln!();
    result
}
pub(super) fn run(args: &[String]) -> Result<i32> {
    let opts = options(
        args,
        &["--provider", "--url", "--model", "--key-env", "--context-window"],
        &["--confirm", "--key-stdin", "--migrate-builtins"],
    )?;
    ensure!(
        !(opts.contains_key("--key-env") && opts.contains_key("--key-stdin")),
        "choose environment or OS key storage, not both"
    );
    let paths = paths::AppPaths::desktop()?;
    let file = paths.config_dir.join("config.json");
    regular(&file)?;
    let before = if file.try_exists()? { Some(state::read_bounded(&file, 1_000_000)?) } else { None };
    let mut config: Value =
        before.as_ref().map(|v| serde_json::from_slice(v)).transpose()?.unwrap_or(json!({"plugins":"builtin.json"}));
    ensure!(config.is_object(), "API config must be an object");
    let existing = config["protocol"].as_str().unwrap_or("deepseek");
    let provider = opts.get("--provider").cloned().map(Ok).unwrap_or_else(|| {
        prompt(
            "Provider (deepseek / openai / openai-chat / anthropic)",
            if existing == "openai" { "openai-chat" } else { existing },
        )
    })?;
    let (protocol, default_url, default_env) = presets(&provider)?;
    let same = config["protocol"].as_str() == Some(protocol);
    let default_url = if same { config["base_url"].as_str().unwrap_or(default_url) } else { default_url };
    let url = opts.get("--url").cloned().map(Ok).unwrap_or_else(|| {
        let default_url = endpoint(default_url)?;
        if io::stdin().is_terminal() { prompt("URL", &default_url) } else { Ok(default_url) }
    })?;
    let model = opts.get("--model").cloned().map(Ok).unwrap_or_else(|| {
        prompt("Model (required; no paid discovery/probing)", config["model"].as_str().unwrap_or(""))
    })?;
    ensure!(
        !model.trim().is_empty() && model.len() <= 256 && !model.chars().any(char::is_control),
        "model is required and must be a valid identifier"
    );
    config["protocol"] = json!(protocol);
    config["base_url"] = json!(endpoint(&url)?);
    config["model"] = json!(model);
    if before.is_none() {
        config["max_tokens"] = json!(1024);
    }
    if let Some(window) = opts.get("--context-window") {
        let window: u64 = window.parse().context("invalid context window")?;
        ensure!(
            window > config["max_tokens"].as_u64().unwrap_or(8192).saturating_add(512) && window <= 1_000_000_000_000,
            "context window is too small or implausible"
        );
        ensure!(config["budget"].is_null() || config["budget"].is_object(), "existing budget is not an object");
        if config["budget"].is_null() {
            config["budget"] = json!({});
        }
        config["budget"]["enabled"] = json!(true);
        config["budget"]["context_window"] = json!(window);
    } else if config.get("budget").is_none() {
        config["budget"] = json!({"enabled":false});
    }
    let mut store = opts.contains_key("--key-stdin");
    if store {
        ensure!(!io::stdin().is_terminal(), "--key-stdin requires a pipe, never type a key into an echoing terminal");
    }
    let key_env = opts.get("--key-env").cloned().unwrap_or_else(|| {
        if same { config["api_key_env"].as_str().unwrap_or(default_env).to_owned() } else { default_env.to_owned() }
    });
    ensure!(env_name(&key_env), "invalid API key environment variable name");
    if !store && !opts.contains_key("--key-env") && io::stdin().is_terminal() {
        let mode = prompt("Key storage (env / store)", if env::var_os(&key_env).is_some() { "env" } else { "store" })?;
        ensure!(matches!(mode.as_str(), "env" | "store"), "choose env or store");
        store = mode == "store";
    }
    config["api_key_env"] = json!(key_env);
    if opts.contains_key("--migrate-builtins") {
        ensure!(
            config["plugins"] == "desktop.json",
            "migration only supports the legacy desktop.json reference; custom manifests are preserved"
        );
        let old: Value =
            serde_json::from_slice(&state::read_bounded(&paths.config_dir.join("desktop.json"), 1_000_000)?)?;
        ensure!(
            generated_legacy(&old, &desktop::manifest(&paths, None)?),
            "legacy manifest contains custom changes; inspect it manually, nothing replaced"
        );
        config["plugins"] = json!("builtin.json");
    }
    eprintln!(
        "Provider: {protocol}\nURL: {}\nModel: {}\nConfig: {}",
        config["base_url"],
        config["model"],
        file.display()
    );
    if before.is_some() {
        ensure!(
            opts.contains_key("--confirm")
                || confirm("Update connection settings, preserve advanced fields and keep a backup?")?,
            "existing config preserved; rerun with --confirm"
        );
    }
    std::fs::create_dir_all(&paths.config_dir)?;
    let _lock = state::Lock::acquire(&paths.config_dir.join("setup.lock"))?;
    regular(&file)?;
    let current = if file.try_exists()? { Some(state::read_bounded(&file, 1_000_000)?) } else { None };
    ensure!(current == before, "config changed concurrently; retry setup");
    let marker = json!({"builtin_profile":"configured","tools":[]});
    let builtin = paths.config_dir.join("builtin.json");
    regular(&builtin)?;
    if config["plugins"] == "builtin.json" {
        if builtin.try_exists()? {
            ensure!(
                serde_json::from_slice::<Value>(&state::read_bounded(&builtin, 1_000_000)?)? == marker,
                "custom builtin.json preserved; review before setup"
            );
        }
        desktop::create(&builtin, &marker)?;
    }
    for (name, value) in [
        ("plugins.json", json!({"profile":"standard","disabled":[]})),
        ("bindings.json", json!({})),
        ("provider.json", json!({"images":"base64"})),
        ("cache.json", json!({"mode":"observe","target_hit_ratio":0.99})),
    ] {
        desktop::create(&paths.config_dir.join(name), &value)?;
    }
    let reference = if store {
        let key = if opts.contains_key("--key-stdin") {
            let mut bytes = Vec::new();
            io::stdin().take(8193).read_to_end(&mut bytes)?;
            ensure!(bytes.len() <= 8192, "key input too long");
            String::from_utf8(bytes)?.trim_end_matches(['\r', '\n']).to_owned()
        } else {
            secret()?
        };
        let reference = super::super::credentials::store(&key)?;
        config["api_key_env"] = json!(reference);
        Some(reference)
    } else {
        None
    };
    let saved = (|| -> Result<()> {
        regular(&file)?;
        let current = if file.try_exists()? { Some(state::read_bounded(&file, 1_000_000)?) } else { None };
        ensure!(current == before, "config changed during credential entry; existing config preserved");
        if before.is_some() {
            eprintln!("Config backup: {}", backup(&file)?.display());
        }
        state::atomic(&file, &config)
    })();
    if saved.is_err()
        && let Some(reference) = reference
        && super::super::credentials::remove(&reference).is_err()
    {
        eprintln!("OS credential rollback failed; remove unused reference {reference} from OS storage manually.");
    }
    saved?;
    if config["budget"]["enabled"] == false {
        eprintln!(
            "Context budget/automatic compaction disabled explicitly: no verified context size was supplied. Use --context-window to enable; transport, I/O and approval limits remain active."
        );
    }
    if config["plugins"] != "builtin.json" {
        eprintln!(
            "Custom/legacy plugin manifest preserved. Review development paths; --migrate-builtins can migrate an unchanged generated desktop manifest."
        );
    }
    if !store && env::var_os(&key_env).is_none() && !key_env.starts_with("I_CREDENTIAL_") {
        eprintln!("Set {key_env} privately before making requests; its value was not stored.");
    }
    println!("Configured. No model requests made. Run i start (or bare i in a terminal).");
    Ok(0)
}
