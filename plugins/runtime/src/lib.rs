use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    env,
    io::{self, BufRead, Read, Write},
    path::{Path, PathBuf},
};

mod approval;
#[cfg(test)]
mod approval_tests;
mod brand;
mod budget;
mod cache;
mod compact;
#[cfg(test)]
mod compact_tests;
mod connection_job;
mod connections;
mod console;
mod console_editor;
mod console_ui;
mod context;
mod credentials;
mod deployment;
mod desktop;
mod installation;
mod model_settings;
mod proxy;
pub fn model_proxy() -> Result<Option<String>> {
    proxy::model()
}
pub fn apply_connection_proxy(command: &mut std::process::Command, id: &str) -> Result<()> {
    proxy::apply_connection(command, id)
}
static HOSTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn entry(args: Vec<String>) -> Result<()> {
    use std::io::IsTerminal;
    HOSTED.store(true, std::sync::atomic::Ordering::Relaxed);
    match args.first().map(String::as_str) {
        Some("--runtime") => main(args.into_iter().skip(1).collect()),
        Some("install" | "setup" | "start" | "plugins" | "login") => main(args),
        None if io::stdin().is_terminal() && io::stdout().is_terminal() => main(vec!["start".into()]),
        _ => Ok(()),
    }
}
pub fn deployment_key(name: &str, current: String) -> Result<String> {
    if !current.is_empty() || !name.starts_with("I_CREDENTIAL_") {
        return Ok(current);
    }
    credentials::load(name)
}
pub fn check_connection(provider: &Value) -> Result<()> {
    if provider.get("managed_connection").is_none() {
        return Ok(());
    }
    connections::check_connection(&paths::AppPaths::desktop()?, provider)
}
pub fn resolve_manifest(value: Value) -> Result<Value> {
    if value.get("builtin_profile").is_none() {
        if value["tools"].as_array().is_some_and(|tools| tools.iter().any(|tool| tool["name"] == "search")) {
            return desktop::resolve_legacy(value, &paths::AppPaths::desktop()?);
        }
        return Ok(value);
    }
    ensure!(
        value.as_object().is_some_and(|v| v.keys().all(|k| matches!(k.as_str(), "builtin_profile" | "tools")))
            && value["tools"] == json!([]),
        "invalid portable builtin manifest"
    );
    let profile = text(&value, "builtin_profile")?;
    desktop::manifest(&paths::AppPaths::desktop()?, if profile == "configured" { None } else { Some(profile) })
}
mod clipboard;
mod markdown;
mod memory;
#[cfg(test)]
mod optimization_tests;
mod paths;
mod prompts;
mod provider;
mod provider_files;
#[cfg(test)]
mod provider_tests;
mod reflection;
#[cfg(test)]
mod reflection_tests;
mod responses;
mod search;
mod skills;
mod smoke;
mod state;
mod steering;
mod terminal_mode;
#[cfg(test)]
mod tests;
mod web;
const MODULES: &[&str] = &[
    "provider",
    "approval",
    "budget",
    "compact",
    "reflection",
    "cache",
    "responses",
    "memory",
    "prompts",
    "search",
    "web",
    "skills",
    "steering",
    "brand",
    "smoke",
];

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key].as_str().with_context(|| format!("missing string: {key}"))
}
fn option(args: &[String], flag: &str) -> Option<String> {
    args.iter().enumerate().find_map(|(i, arg)| {
        arg.strip_prefix(&format!("{flag}="))
            .map(str::to_owned)
            .or_else(|| (arg == flag).then(|| args.get(i + 1).cloned()).flatten())
    })
}
fn configured(args: &[String]) -> Result<(Value, PathBuf)> {
    match option(args, "--config") {
        Some(path) => {
            let path = dunce::canonicalize(path)?;
            Ok((serde_json::from_slice(&state::read_bounded(&path, 256_000)?)?, path.parent().unwrap().into()))
        }
        None => Ok((json!({}), env::current_dir()?)),
    }
}
fn handle(command: &str, event: Value, args: &[String]) -> Result<Value> {
    ensure!(event["data"].is_object(), "event data must be an object");
    let kind = text(&event, "event")?;
    let workspace = event["workspace"].as_str().unwrap_or(".");
    let locations: paths::AppPaths = match event.get("paths") {
        Some(value) => serde_json::from_value(value.clone())?,
        None => paths::AppPaths::desktop()?,
    };
    locations.validate()?;
    let mut resolved = args.to_vec();
    if option(args, "--config").is_none() {
        let file = locations.config_dir.join(format!("{command}.json"));
        if file.try_exists()? {
            resolved.extend(["--config".into(), file.to_string_lossy().into_owned()]);
        }
    }
    let args = resolved.as_slice();
    let store = if matches!(command, "memory" | "cache") {
        locations.workspace(Path::new(workspace), event["workspace_id"].as_str())?.1
    } else {
        PathBuf::from(workspace)
    };
    let data = event["data"].clone();
    match command {
        "provider" => provider_files::handle(kind, data, args, &locations, workspace, event["workspace_id"].as_str()),
        "approval" => approval::handle(kind, data, args),
        "budget" => budget::handle(kind, data),
        "compact" => compact::handle(kind, data, workspace, args),
        "reflection" => reflection::handle(kind, data, args),
        "smoke" => smoke::handle(kind, data, args),
        "cache" => cache::handle(kind, data, &store, args),
        "brand" => Ok(json!({"output":brand::render(false,false)})),
        "responses" => responses::handle(kind, data),
        "memory" => memory::handle(kind, data, &store),
        "prompts" => prompts::handle(kind, data, workspace, args),
        "search" => search::handle(kind, data, Path::new(workspace)),
        "skills" => skills::handle(kind, data, Path::new(workspace), args),
        "web" => web::handle(kind, data, args),
        "steering" => {
            steering::handle(kind, data, Path::new(&option(args, "--queue").context("steering requires --queue")?))
        }
        _ => bail!("unknown plugin: {command}"),
    }
}
pub(crate) fn executable() -> Result<Vec<String>> {
    let mut argv = vec![env::current_exe()?.to_str().context("executable path must be UTF-8")?.to_owned()];
    if HOSTED.load(std::sync::atomic::Ordering::Relaxed) || env::args().nth(1).as_deref() == Some("--runtime") {
        argv.push("--runtime".into());
    }
    Ok(argv)
}
fn run(args: Vec<String>) -> Result<i32> {
    let command = args
        .first()
        .context("usage: i-plugins console|init|setup|install|start|paths|logo|manifest|cache-report|provider-files; worker modules: provider|approval|budget|compact|reflection|cache|responses|memory|prompts|search|web|skills|steering|brand|smoke")?;
    if matches!(command.as_str(), "plugins" | "login") {
        return connections::run_cli(command, &args[1..]);
    }
    if matches!(command.as_str(), "install" | "setup" | "start") {
        return deployment::run(command, &args[1..]);
    }
    if command == "console" {
        return console::run(&args[1..]);
    }
    if command == "logo" {
        if args.iter().any(|arg| arg == "--svg") {
            println!("{}", brand::svg());
            return Ok(0);
        }
        use std::io::IsTerminal;
        print!(
            "{}",
            brand::render(
                io::stdout().is_terminal() && env::var_os("NO_COLOR").is_none() && !args.iter().any(|a| a == "--plain"),
                args.iter().any(|a| a == "--ascii")
            )
        );
        return Ok(0);
    }
    if matches!(command.as_str(), "paths" | "cache-report" | "provider-files" | "init") {
        let locations = paths::AppPaths::desktop()?;
        let root = option(&args, "-C").unwrap_or_else(|| ".".into());
        if command == "init" {
            return desktop::initialize(&locations);
        }
        let value = if command == "provider-files" {
            provider_files::registry(&locations.workspace(Path::new(&root), None)?.1, &args[1..])?
        } else if command == "paths" {
            locations.describe(Path::new(&root))?
        } else {
            let mut options = args[1..].to_vec();
            let default = locations.config_dir.join("cache.json");
            if option(&options, "--config").is_none() && default.try_exists()? {
                options.extend(["--config".into(), default.to_string_lossy().into_owned()]);
            }
            cache::report(&locations.workspace(Path::new(&root), None)?.1, &options)?
        };
        serde_json::to_writer_pretty(io::stdout().lock(), &value)?;
        return Ok(0);
    }
    if command == "manifest" {
        let locations = paths::AppPaths::desktop()?;
        std::fs::create_dir_all(&locations.config_dir)?;
        let manifest = desktop::manifest(&locations, args.get(1).map(String::as_str))?;
        serde_json::to_writer_pretty(io::stdout().lock(), &manifest)?;
        return Ok(0);
    }
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    loop {
        let mut bytes = Vec::new();
        input.by_ref().take(48 * 1024 * 1024 + 1).read_until(b'\n', &mut bytes)?;
        if bytes.is_empty() {
            break;
        }
        ensure!(bytes.len() <= 48 * 1024 * 1024 && bytes.last() == Some(&b'\n'), "invalid/oversized worker frame");
        let event: Value = serde_json::from_slice(&bytes)?;
        let id = event["id"].as_u64().context("worker request needs unsigned id")?;
        let reply = match handle(command, event, &args[1..]) {
            Ok(data) => json!({"id":id,"data":data}),
            Err(error) => json!({"id":id,"error":format!("{error:#}")}),
        };
        let bytes = serde_json::to_vec(&reply)?;
        ensure!(bytes.len() < 48 * 1024 * 1024, "worker output exceeds 48 MiB");
        output.write_all(&bytes)?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
    Ok(0)
}
pub fn main(args: Vec<String>) -> ! {
    match run(args) {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("plugin: {error:#}");
            std::process::exit(1);
        }
    }
}
