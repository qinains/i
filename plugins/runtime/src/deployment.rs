//! User-level deployment; no model or provider API calls.
use super::*;
#[path = "install.rs"]
mod install;
#[path = "setup.rs"]
mod setup;
#[cfg(test)]
#[path = "deployment_tests.rs"]
mod tests;
pub(crate) fn run(command: &str, args: &[String]) -> Result<i32> {
    if args.iter().any(|a| a == "--help") {
        println!(
            "i install [--bin-dir DIR] [--replace] [--add-to-path] [--sha256 HASH]\ni setup [--provider deepseek|openai|openai-chat|anthropic] [--url URL] [--model MODEL] [--key-env NAME | --key-stdin] [--context-window N] [--confirm] [--migrate-builtins]\ni start [console / core options]\nSetup preserves advanced settings, backs up changes, and never probes a paid API. --key-stdin requires a pipe and stores the key in OS credential storage, not argv/config JSON."
        );
        return Ok(0);
    }
    match command {
        "install" => install::run(args),
        "setup" => setup::run(args),
        "start" => {
            let paths = paths::AppPaths::desktop()?;
            use std::io::IsTerminal;
            if crate::HOSTED.load(std::sync::atomic::Ordering::Relaxed)
                && io::stdin().is_terminal()
                && io::stdout().is_terminal()
                && option(args, "--config").is_none()
                && let Err(error) = offer_install(&paths.config_dir)
            {
                eprintln!(
                    "Optional installation did not complete: {error:#}\nContinuing without installation. You can run i install --add-to-path later."
                );
            }
            let config =
                option(args, "--config").map(PathBuf::from).unwrap_or_else(|| paths.config_dir.join("config.json"));
            let value = if config.try_exists()? {
                serde_json::from_slice::<Value>(&state::read_bounded(&config, 1_000_000)?)?
            } else {
                json!({})
            };
            if (option(args, "--model").is_none()
                && value["model"].as_str().is_none()
                && env::var_os("API_MODEL").is_none())
                || (option(args, "--base-url").is_none()
                    && value["base_url"].as_str().is_none()
                    && env::var_os("API_BASE_URL").is_none())
            {
                use std::io::IsTerminal;
                ensure!(
                    option(args, "--config").is_none(),
                    "explicit API config is incomplete; configure it rather than replacing it"
                );
                ensure!(io::stdin().is_terminal(), "run i setup first (model and endpoint are required)");
                let mut exec = super::executable()?;
                exec.push("setup".into());
                let status = std::process::Command::new(&exec[0]).args(&exec[1..]).status()?;
                ensure!(status.success(), "setup did not complete");
            }
            console::run(args)
        }
        _ => bail!("unknown deployment command"),
    }
}
fn first_run_once(
    config_dir: &Path,
    ask: impl FnOnce() -> Result<bool>,
    install: impl FnOnce() -> Result<()>,
) -> Result<()> {
    std::fs::create_dir_all(config_dir)?;
    let _lock = state::Lock::acquire(&config_dir.join(".first-run-install.lock"))?;
    let marker = config_dir.join("first-run-install.json");
    regular(&marker)?;
    if marker.try_exists()? {
        return Ok(());
    }
    let accepted = ask()?;
    if accepted {
        install()?;
    }
    state::atomic(&marker, &json!({"version": 1, "installed": accepted}))
}
fn offer_install(config_dir: &Path) -> Result<()> {
    let source = dunce::canonicalize(env::current_exe()?)?;
    let target = install::default_dir()?.join(format!("i{}", env::consts::EXE_SUFFIX));
    let resolved = env::var_os("PATH").and_then(|path| {
        env::split_paths(&path).map(|dir| dir.join(format!("i{}", env::consts::EXE_SUFFIX))).find(|p| p.is_file())
    });
    if dunce::canonicalize(&target).ok().as_ref() == Some(&source)
        || resolved.and_then(|p| dunce::canonicalize(p).ok()).as_ref() == Some(&source)
    {
        return Ok(());
    }
    first_run_once(
        config_dir,
        || {
            eprintln!(
                "Optional: install i to {} and add it to your user PATH?\nOnly your user account is changed; existing files require confirmation.\nChoose no to keep using this extracted executable. Setup and the session continue either way.",
                target.display()
            );
            eprint!("Install for this user? [y/N] ");
            io::stderr().flush()?;
            let mut line = String::new();
            ensure!(io::stdin().lock().take(1024).read_line(&mut line)? > 0, "installation prompt closed");
            Ok(line.trim().eq_ignore_ascii_case("y"))
        },
        || {
            ensure!(install::run(&["--add-to-path".into()])? == 0, "installation failed");
            Ok(())
        },
    )
}
fn options(args: &[String], values: &[&str], switches: &[&str]) -> Result<std::collections::BTreeMap<String, String>> {
    let mut out = std::collections::BTreeMap::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let (name, inline) = arg.split_once('=').map_or((arg.as_str(), None), |(k, v)| (k, Some(v)));
        ensure!(values.contains(&name) || switches.contains(&name), "unknown deployment option (see --help)");
        ensure!(!out.contains_key(name), "duplicate option: {name}");
        let value = if values.contains(&name) {
            inline.or_else(|| iter.next().map(String::as_str)).context("missing option value")?
        } else {
            ensure!(inline.is_none(), "switch does not take a value");
            "true"
        };
        ensure!(!value.is_empty() && !value.starts_with("--"), "missing option value: {name}");
        out.insert(name.to_owned(), value.to_owned());
    }
    Ok(out)
}
fn backup(path: &Path) -> Result<PathBuf> {
    let dest = path.with_file_name(format!(
        "{}.backup-{}",
        path.file_name().context("missing filename")?.to_string_lossy(),
        uuid::Uuid::new_v4().simple()
    ));
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&dest)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(&state::read_bounded(path, 64 * 1024 * 1024)?)?;
    file.sync_all()?;
    Ok(dest)
}
fn confirm(question: &str) -> Result<bool> {
    use std::io::IsTerminal;
    if !io::stdin().is_terminal() {
        return Ok(false);
    }
    eprint!("{question} [y/N] ");
    io::stderr().flush()?;
    let mut line = String::new();
    io::stdin().lock().take(1024).read_line(&mut line)?;
    Ok(line.trim().eq_ignore_ascii_case("y"))
}
fn regular(path: &Path) -> Result<()> {
    if path.try_exists()? || path.is_symlink() {
        ensure!(
            std::fs::symlink_metadata(path)?.file_type().is_file(),
            "refusing non-regular/symlink target: {}",
            path.display()
        );
    }
    Ok(())
}
