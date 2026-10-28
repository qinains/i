//! Desktop-only executable bindings. Portable plugin settings contain no argv paths.
use super::{paths::AppPaths, state};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{fs, io::Write, path::Path};

fn legacy_module<'a>(entry: &'a Value, paths: &AppPaths) -> Option<&'a str> {
    if entry["cwd"].as_str() != paths.config_dir.to_str() {
        return None;
    }
    let exec = entry["exec"].as_array()?;
    if exec.len() != 2 {
        return None;
    }
    let program = Path::new(exec[0].as_str()?);
    if !program.is_absolute() || program.file_stem()?.to_str()? != "i-plugins" {
        return None;
    }
    let module = exec[1].as_str()?;
    super::MODULES.contains(&module).then_some(module)
}
pub(crate) fn resolve_legacy(mut value: Value, paths: &AppPaths) -> Result<Value> {
    let obsolete = |entry: &Value| {
        legacy_module(entry, paths) == Some("search") && matches!(entry["name"].as_str(), Some("search" | "find_files"))
    };
    if !value["tools"].as_array().is_some_and(|tools| tools.iter().any(obsolete)) {
        return Ok(value);
    }
    // Only the retired user-config builtins are recognized. Custom name collisions
    // still reach the core's normal rejection, and the on-disk manifest is untouched.
    value["tools"].as_array_mut().unwrap().retain(|entry| !obsolete(entry));
    let bindings = bindings(paths)?;
    let rewrite = |entry: &mut Value| -> Result<()> {
        if let Some(module) = legacy_module(entry, paths).map(str::to_owned) {
            let mut argv = super::executable()?;
            argv.push(module.clone());
            entry["exec"] = bindings[&module].get("exec").cloned().unwrap_or_else(|| json!(argv));
            if let Some(cwd) = bindings[&module].get("cwd") {
                entry["cwd"] = cwd.clone();
            }
        }
        Ok(())
    };
    for section in ["tools", "hooks", "analyses"] {
        if let Some(entries) = value.get_mut(section).and_then(Value::as_array_mut) {
            for entry in entries {
                rewrite(entry)?;
            }
        }
    }
    if let Some(provider) = value.get_mut("provider") {
        rewrite(provider)?;
    }
    Ok(value)
}

#[cfg(test)]
#[path = "desktop_legacy_tests.rs"]
mod legacy_tests;

fn selection(paths: &AppPaths) -> Result<Value> {
    let file = paths.config_dir.join("plugins.json");
    Ok(if file.try_exists()? {
        serde_json::from_slice(&state::read_bounded(&file, 256_000)?)?
    } else {
        json!({"profile":"standard","disabled":[]})
    })
}
pub(crate) fn bindings(paths: &AppPaths) -> Result<Value> {
    let file = paths.config_dir.join("bindings.json");
    let bindings: Value =
        if file.try_exists()? { serde_json::from_slice(&state::read_bounded(&file, 256_000)?)? } else { json!({}) };
    for (module, binding) in bindings.as_object().context("bindings must be an object")? {
        ensure!(super::MODULES.contains(&module.as_str()), "unknown built-in module binding: {module}");
        let fields = binding.as_object().context("binding must be an object")?;
        ensure!(fields.keys().all(|key| matches!(key.as_str(), "exec" | "cwd")), "binding supports only exec and cwd");
        let exec = binding["exec"].as_array().context("binding.exec must be an array")?;
        ensure!(!exec.is_empty() && exec.iter().all(Value::is_string), "binding.exec must contain strings");
        ensure!(Path::new(exec[0].as_str().unwrap()).is_absolute(), "binding executable must be absolute");
        if let Some(cwd) = binding.get("cwd") {
            ensure!(cwd.as_str().is_some_and(|s| Path::new(s).is_absolute()), "binding cwd must be absolute");
        }
    }
    Ok(bindings)
}
pub(crate) fn manifest(paths: &AppPaths, profile: Option<&str>) -> Result<Value> {
    let selection = selection(paths)?;
    ensure!(
        selection.is_object() && selection.get("profile").is_none_or(Value::is_string),
        "invalid portable plugin selection"
    );
    let profile = profile.or_else(|| selection["profile"].as_str()).unwrap_or("standard");
    let source = match profile {
        "full" | "standard" => include_str!("../../full.json"),
        "approval" => include_str!("../../approval.json"),
        "budget" => include_str!("../../budget.json"),
        "compact" => include_str!("../../compact.json"),
        "reflection" => include_str!("../../reflection.json"),
        "cache" => include_str!("../../cache.json"),
        "responses" => include_str!("../../responses.json"),
        "responses-budget" => include_str!("../../responses-budget.json"),
        "responses-compact" => include_str!("../../responses-compact.json"),
        "memory" => include_str!("../../memory.json"),
        "search" => include_str!("../../search.json"),
        "skills" => include_str!("../../skills.json"),
        "web" => include_str!("../../web.json"),
        _ => bail!("unknown manifest profile"),
    };
    let bindings = bindings(paths)?;
    let mut value: Value = serde_json::from_str(source)?;
    let disabled = selection.get("disabled").cloned().unwrap_or(json!([]));
    let disabled = disabled.as_array().context("disabled must be an array")?;
    ensure!(disabled.iter().all(Value::is_string), "disabled entries must be module names");
    if value.get("analyses").is_none() {
        value["analyses"] = json!([]);
    }
    for section in ["tools", "hooks", "analyses"] {
        let entries = value[section].as_array_mut().context("invalid manifest")?;
        entries.retain(|entry| {
            !(disabled.contains(&entry["exec"][1]) || profile == "standard" && entry["exec"][1] == "responses")
        });
        for entry in entries {
            let module = entry["exec"][1].as_str().context("missing worker module")?.to_owned();
            let mut argv = super::executable()?;
            argv.push(module.clone());
            let binding = &bindings[&module];
            entry["exec"] = binding.get("exec").cloned().unwrap_or_else(|| json!(argv));
            entry["cwd"] = binding.get("cwd").cloned().unwrap_or_else(|| json!(paths.config_dir));
            entry["module"] = json!(module); // Diagnostic identity, not sent as model tool schema.
        }
    }
    let mut argv = super::executable()?;
    argv.push("provider".into());
    value["provider"] = json!({"exec":bindings["provider"].get("exec").cloned().unwrap_or(json!(argv)),
        "cwd":bindings["provider"].get("cwd").cloned().unwrap_or(json!(paths.config_dir)),"module":"provider"});
    Ok(value)
}
pub(crate) fn create(path: &Path, value: &Value) -> Result<()> {
    if path.try_exists()? {
        return Ok(());
    }
    let mut file = tempfile::NamedTempFile::new_in(path.parent().context("missing directory")?)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    file.as_file().sync_all()?;
    match file.persist_noclobber(path) {
        Ok(_) => Ok(()),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error.into()),
    }
}
pub(crate) fn initialize(paths: &AppPaths) -> Result<i32> {
    fs::create_dir_all(&paths.config_dir)?;
    create(&paths.config_dir.join("plugins.json"), &json!({"profile":"standard","disabled":[]}))?;
    create(&paths.config_dir.join("bindings.json"), &json!({}))?;
    create(&paths.config_dir.join("builtin.json"), &json!({"builtin_profile":"configured","tools":[]}))?;
    let plugins = if super::HOSTED.load(std::sync::atomic::Ordering::Relaxed) {
        "builtin.json"
    } else {
        create(&paths.config_dir.join("desktop.json"), &manifest(paths, None)?)?;
        "desktop.json"
    };
    create(&paths.config_dir.join("config.json"), &json!({"plugins":plugins}))?;
    create(&paths.config_dir.join("cache.json"), &json!({"mode":"observe","target_hit_ratio":0.99}))?;
    create(&paths.config_dir.join("provider.json"), &json!({"images":"base64"}))?;
    println!(
        "Configuration: {}\nExisting files were preserved. Configure model/endpoint and credentials separately.",
        paths.config_dir.display()
    );
    Ok(0)
}
