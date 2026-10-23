use super::{
    approval, brand,
    console_ui::{Input, Ui},
    option,
    paths::AppPaths,
    state, steering,
};
use anyhow::{Context, Result, ensure};
use command_group::{CommandGroup, GroupChild};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    env, fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

fn without(args: &[String], flag: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == flag {
            iter.next();
        } else if !arg.starts_with(&format!("{flag}=")) {
            result.push(arg.clone());
        }
    }
    result
}
fn validate_args(args: &[String]) -> Result<()> {
    ensure!(!args.iter().any(|arg| arg == "--yes"), "--yes was removed; use --yolo or --dangerously-skip-permissions");
    let valued = [
        "--bin",
        "--task",
        "--config",
        "--plugins",
        "-C",
        "--dir",
        "--session",
        "--session-name",
        "--api-key-env",
        "--model",
        "-m",
        "--base-url",
        "--protocol",
        "--image",
        "--max-tokens",
        "--max-steps",
        "--timeout",
        "--sessions",
    ];
    for (index, arg) in args.iter().enumerate() {
        let (name, inline) = arg.split_once('=').map_or((arg.as_str(), None), |(name, value)| (name, Some(value)));
        if valued.contains(&name) {
            let value =
                inline.or_else(|| args.get(index + 1).map(String::as_str).filter(|value| !value.starts_with('-')));
            ensure!(
                value.is_some_and(|value| !value.is_empty()),
                "{name} requires a value; use {name}=VALUE. Shell newlines need a trailing backslash."
            );
        }
    }
    Ok(())
}
fn manifest(args: &[String], inbox: &Path, mode: &Path, paths: &AppPaths) -> Result<Value> {
    let source = if let Some(path) = option(args, "--plugins") {
        Some(PathBuf::from(path))
    } else if let Some(path) = option(args, "--config") {
        let path = dunce::canonicalize(&path).with_context(|| format!("open API config {path}"))?;
        let config: Value = serde_json::from_slice(&state::read_bounded(&path, 1_000_000)?)?;
        config["plugins"].as_str().map(|name| path.parent().unwrap().join(name))
    } else {
        None
    };
    let mut manifest = json!({"tools":[],"hooks":[]});
    if let Some(path) = source {
        let path = dunce::canonicalize(&path).with_context(|| format!("open plugin manifest {}", path.display()))?;
        manifest = super::resolve_manifest(serde_json::from_slice(&state::read_bounded(&path, 1_000_000)?)?)?;
        if let Some(provider) = manifest.get_mut("provider") {
            ensure!(provider.is_object(), "provider must be a binding");
            provider["cwd"] = json!(path.parent().unwrap().join(provider["cwd"].as_str().unwrap_or("")));
        }
        for section in ["tools", "hooks"] {
            if manifest.get(section).is_none() {
                manifest[section] = json!([]);
            }
            for entry in manifest[section].as_array_mut().context("invalid plugin manifest")? {
                entry["cwd"] = json!(path.parent().unwrap().join(entry["cwd"].as_str().unwrap_or("")));
            }
        }
    }
    let bindings = super::desktop::bindings(paths)?;
    if let Some(binding) = bindings.get("provider") {
        let mut provider = binding.clone();
        if provider.get("cwd").is_none() {
            provider["cwd"] = json!(paths.config_dir);
        }
        manifest["provider"] = provider;
    }
    let binding = &bindings["steering"];
    let mut argv = if let Some(exec) = binding["exec"].as_array() {
        exec.iter().map(|arg| arg.as_str().unwrap().to_owned()).collect()
    } else {
        let mut argv = super::executable()?;
        argv.push("steering".into());
        argv
    };
    argv.extend(["--queue".into(), inbox.to_str().context("queue path must be UTF-8")?.into()]);
    manifest["hooks"].as_array_mut().unwrap().push(json!({"events":["before_turn","after_save"],"exec":argv,
        "cwd":binding.get("cwd").cloned().unwrap_or_else(|| json!(inbox.parent().unwrap())),"module":"steering"}));
    let hooks = manifest["hooks"].as_array_mut().context("invalid hooks")?;
    if !hooks.iter().any(|hook| hook["module"] == "approval") {
        let mut exec = super::executable()?;
        exec.push("approval".into());
        // Custom hooks run afterwards and may still require a prompt or deny in auto mode.
        hooks.insert(0, json!({"events":["tool_approval"],"exec":exec,"cwd":mode.parent(),"module":"approval"}));
    }
    ensure!(
        hooks.iter().any(|hook| hook["module"] == "approval"
            && hook["events"]
                .as_array()
                .is_some_and(|events| events.iter().any(|event| event == "tool_approval" || event == "*"))),
        "approval module must handle tool_approval"
    );
    for hook in hooks.iter_mut().filter(|hook| hook["module"] == "approval") {
        let binding = &bindings["approval"];
        if let Some(exec) = binding.get("exec") {
            hook["exec"] = exec.clone();
        }
        if let Some(cwd) = binding.get("cwd") {
            hook["cwd"] = cwd.clone();
        }
        hook["exec"]
            .as_array_mut()
            .context("invalid approval exec")?
            .extend([json!("--mode-file"), json!(mode.to_str().context("mode path must be UTF-8")?)]);
    }
    Ok(manifest)
}
pub(crate) fn relay(
    mut input: impl Read,
    mut output: impl Write,
    approval: &AtomicBool,
    token: Option<&str>,
) -> io::Result<()> {
    let mut frame: Option<Vec<u8>> = None;
    let mut bytes = [0; 8192];
    loop {
        let n = input.read(&mut bytes)?;
        if n == 0 {
            break;
        }
        let mut visible = Vec::with_capacity(n);
        for &byte in &bytes[..n] {
            if token.is_some() && byte == 0x1e {
                match frame.take() {
                    Some(frame) => {
                        if Some(frame.as_slice()) == token.map(str::as_bytes) {
                            approval.store(true, Ordering::Release);
                        }
                    }
                    None => frame = Some(Vec::new()),
                }
            } else if let Some(buffer) = &mut frame {
                buffer.push(byte);
                if buffer.len() > 128 {
                    frame = None;
                }
            } else {
                visible.push(byte);
            }
        }
        output.write_all(&visible)?;
        output.flush()?;
    }
    Ok(())
}
struct Running {
    child: GroupChild,
    readers: Vec<thread::JoinHandle<()>>,
    exited: Option<(i32, Instant)>,
    _images: Vec<tempfile::NamedTempFile>,
}
impl Running {
    fn poll(&mut self) -> Result<Option<i32>> {
        if self.exited.is_none()
            && let Some(exit) = self.child.try_wait()?
        {
            self.exited = Some((exit.code().unwrap_or(1), Instant::now()));
            let _ = self.child.kill();
        }
        Ok(self
            .exited
            .filter(|(_, time)| {
                self.readers.iter().all(|r| r.is_finished()) || time.elapsed() >= Duration::from_secs(2)
            })
            .map(|(code, _)| code))
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let deadline = self.exited.map_or_else(Instant::now, |(_, time)| time) + Duration::from_secs(2);
        while self.readers.iter().any(|reader| !reader.is_finished()) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        for reader in self.readers.drain(..).filter(|reader| reader.is_finished()) {
            let _ = reader.join();
        }
    }
}
struct Task {
    text: String,
    images: Vec<(tempfile::NamedTempFile, String)>,
}
impl From<String> for Task {
    fn from(text: String) -> Self {
        Self { text, images: Vec::new() }
    }
}
impl From<&str> for Task {
    fn from(text: &str) -> Self {
        text.to_owned().into()
    }
}
fn image_task(text: String, images: Vec<Vec<u8>>, args: &[String]) -> Result<Task> {
    ensure!(!text.trim_start().starts_with('/'), "Attachments cannot be sent with slash commands");
    ensure!(
        images.len() <= 8 && images.iter().map(Vec::len).sum::<usize>() <= 16 * 1024 * 1024,
        "Image limits exceeded"
    );
    let root = dunce::canonicalize(option(args, "-C").or_else(|| option(args, "--dir")).unwrap_or_else(|| ".".into()))?;
    let directory = state::inside(&root, Path::new(".i/attachments"))?;
    fs::create_dir_all(&directory)?;
    ensure!(dunce::canonicalize(&directory)?.starts_with(&root), "Attachment directory escapes workspace");
    let mut files = Vec::new();
    for image in images {
        ensure!(
            image.len() <= 8 * 1024 * 1024 && image.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]),
            "Invalid clipboard PNG"
        );
        let mut file = tempfile::Builder::new().prefix("paste-").suffix(".png").tempfile_in(&directory)?;
        file.write_all(&image)?;
        file.as_file().sync_all()?;
        let relative = file.path().strip_prefix(&root)?.to_str().context("Attachment path must be UTF-8")?.to_owned();
        files.push((file, relative));
    }
    Ok(Task { text, images: files })
}
fn start(binary: &str, args: &[String], task: Task, eof: bool, approval: Arc<AtomicBool>, ui: &Ui) -> Result<Running> {
    let token = uuid::Uuid::new_v4().simple().to_string();
    let mut command = Command::new(binary);
    command.args(args);
    for (_, path) in &task.images {
        command.arg("--image").arg(path);
    }
    let mut child = command
        .arg("--")
        .arg(&task.text)
        .env("I_APPROVAL_TOKEN", &token)
        .stdin(if eof { Stdio::null() } else { Stdio::piped() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .group_spawn()
        .context("start i; build it or use --bin")?;
    let stdout = child.inner().stdout.take().context("missing stdout")?;
    let stderr = child.inner().stderr.take().context("missing stderr")?;
    let out_approval = approval.clone();
    let (out, err) = (ui.sink(false), ui.sink(true));
    let readers = vec![
        thread::spawn(move || {
            let _ = relay(stdout, out, &out_approval, None);
        }),
        thread::spawn(move || {
            let _ = relay(stderr, err, &approval, Some(&token));
        }),
    ];
    Ok(Running { child, readers, exited: None, _images: task.images.into_iter().map(|(f, _)| f).collect() })
}
pub(crate) fn run(args: &[String]) -> Result<i32> {
    if args.iter().any(|arg| arg == "--help") {
        eprintln!(
            "lninl / i console: i-plugins console [--bin PATH] [--task TEXT] [--no-logo] [i options]\nBusy input steers; /queue TEXT follows up; /approve /deny answer approval; /mode ask|auto switches policy; /exit finishes; Ctrl-C aborts."
        );
        return Ok(0);
    }
    validate_args(args)?;
    let sibling = env::current_exe()?.parent().unwrap().join(format!("i{}", env::consts::EXE_SUFFIX));
    let binary = option(args, "--bin")
        .unwrap_or_else(|| if sibling.is_file() { sibling.to_string_lossy().into_owned() } else { "i".into() });
    if !args.iter().any(|arg| arg == "--no-logo") {
        brand::show();
    }
    let paths = AppPaths::desktop()?;
    let mut backend = without(&without(args, "--bin"), "--task");
    let auto = |arg: &str| matches!(arg, "--yolo" | "--dangerously-skip-permissions");
    let initial_mode = if backend.iter().any(|arg| auto(arg)) { "auto" } else { "ask" };
    backend.retain(|arg| arg != "--no-logo" && !auto(arg));
    let default = paths.config_dir.join("config.json");
    if option(&backend, "--config").is_none() && default.try_exists()? {
        backend.extend(["--config".into(), default.to_string_lossy().into_owned()]);
    }
    if option(&backend, "--session").is_none() {
        let name =
            option(&backend, "--session-name").unwrap_or_else(|| format!("console-{}", uuid::Uuid::new_v4().simple()));
        ensure!(
            !name.is_empty()
                && name.len() <= 64
                && name.bytes().all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c)),
            "invalid session name"
        );
        let root = option(&backend, "-C").or_else(|| option(&backend, "--dir")).unwrap_or_else(|| ".".into());
        let (_, store) = paths.workspace(Path::new(&root), None)?;
        let session = state::inside(&store, Path::new(&format!("sessions/{name}.json")))?;
        backend = without(&backend, "--session-name");
        backend.extend(["--session".into(), session.to_string_lossy().into_owned()]);
        eprintln!("[session: {}]", session.display());
    }
    fs::create_dir_all(&paths.runtime_dir)?;
    let directory = tempfile::tempdir_in(&paths.runtime_dir)?;
    let inbox = directory.path().join("inbox.json");
    let mode = directory.path().join("approval-mode.json");
    approval::set_mode(&mode, initial_mode)?;
    let path = directory.path().join("plugins.json");
    fs::write(&path, serde_json::to_vec(&manifest(&backend, &inbox, &mode, &paths)?)?)?;
    backend = without(&backend, "--plugins");
    backend.extend(["--plugins".into(), path.to_string_lossy().into_owned()]);
    let cancelled = Arc::new(AtomicBool::new(false));
    let signal = cancelled.clone();
    ctrlc::set_handler(move || signal.store(true, Ordering::Release))?;
    let mut ui = Ui::new()?;
    let mut followups: VecDeque<Task> = option(args, "--task").into_iter().map(Task::from).collect();
    // Declared after TempDir so process cleanup always precedes directory deletion.
    let mut running: Option<Running> = None;
    let approval = Arc::new(AtomicBool::new(false));
    let (mut eof, mut exiting, mut status) = (false, false, 0);
    ui.note("Type a task; Ctrl-V reads a clipboard image as a pending [Image #N]; add a caption or send it alone. Long text pastes fold without submitting.\nWhile busy: plain text or /steer TEXT steers, /queue TEXT follows up; image messages queue after the current task.\nApprovals: y/n or /approve /deny ONLY when prompted. /mode ask|auto switches policy; /exit finishes; Ctrl-C aborts.")?;
    ui.note(&format!("[mode: {initial_mode}; auto may allow actions without asking; shell is NOT sandboxed]"))?;
    loop {
        ui.drain()?;
        ui.approval_pending(approval.load(Ordering::Acquire))?;
        if cancelled.load(Ordering::Acquire) {
            return Ok(130);
        }
        if let Some(task) = &mut running
            && let Some(exit) = task.poll()?
        {
            status = exit;
            running = None;
            ui.drain()?;
            approval.store(false, Ordering::Release);
            if status == 0 && !exiting && !steering::pending(&inbox)?.is_empty() {
                followups.push_front("Continue with the queued steering instructions.".into());
            } else if status != 0 {
                ui.note(&format!("[task failed: {status}; no automatic task replay; enter a task to continue]"))?;
            }
            if status == 0 {
                ui.note("[task finished]")?;
            }
        }
        if running.is_none() {
            if exiting || (eof && followups.is_empty()) {
                break;
            }
            if let Some(task) = followups.pop_front() {
                let label = if task.text.chars().count() >= 1000 {
                    format!("[Pasted Content {} chars]", task.text.chars().count())
                } else {
                    task.text.clone()
                };
                ui.note(&format!("[task: {label}; {} image(s)]", task.images.len()))?;
                running = Some(start(&binary, &backend, task, eof, approval.clone(), &ui)?);
            }
        }
        let line = match ui.next(Duration::from_millis(50))? {
            Some(Input::Line(line)) => line,
            Some(Input::Clipboard) => {
                match super::clipboard::read_png().and_then(|bytes| ui.stage_image(bytes).map_err(Into::into)) {
                    Ok(()) => {}
                    Err(error) => {
                        ui.note(&format!("[image paste failed: {error:#}; text paste still uses terminal paste]"))?
                    }
                }
                continue;
            }
            Some(Input::Attachments { line, images }) => {
                if exiting || approval.load(Ordering::Acquire) {
                    ui.note("[attachments are not approval answers; send after the pending approval]")?;
                    continue;
                }
                match image_task(line, images, &backend) {
                    Ok(task) => {
                        followups.push_back(task);
                        ui.note("[image message queued; sent after the current task]")?;
                    }
                    Err(error) => ui.note(&format!("[image message rejected: {error:#}]"))?,
                }
                continue;
            }
            Some(Input::Cancel) => return Ok(130),
            Some(Input::Eof) => {
                eof = true;
                if let Some(task) = &mut running {
                    task.child.inner().stdin.take();
                }
                continue;
            }
            None => continue,
        };
        if line.trim().is_empty() {
            continue;
        }
        if ["/queue", "/steer"].contains(&line.trim()) {
            ui.note("[usage: /queue TEXT or /steer TEXT]")?;
            continue;
        }
        let command: Vec<_> = line.split_whitespace().collect();
        if command.first().is_some_and(|name| ["/help", "/approve", "/deny", "/exit"].contains(name))
            && command.len() != 1
        {
            ui.note("[invalid slash command arguments; type /help]")?;
            continue;
        }
        if command.first().copied() == Some("/help") {
            let help = super::console_editor::catalog("console")
                .into_iter()
                .filter(|entry| approval.load(Ordering::Acquire) || !["/approve", "/deny"].contains(&entry.0.as_str()))
                .map(|entry| format!("{}  {}", entry.0, entry.1))
                .collect::<Vec<_>>()
                .join("\n");
            ui.note(&help)?;
            continue;
        }
        if line.starts_with('/')
            && (line.contains(['\n', '\r'])
                || !super::console_editor::catalog("console")
                    .iter()
                    .any(|entry| command.first().copied() == Some(entry.0.as_str())))
        {
            ui.note("[unknown or multiline slash command; type /help]")?;
            continue;
        }
        if command.first().copied() == Some("/mode") {
            let selected = match command.as_slice() {
                ["/mode"] | ["/mode", "status"] => None,
                ["/mode", "auto"] => Some("auto"),
                ["/mode", "ask"] => Some("ask"),
                _ => {
                    ui.note("[usage: /mode [ask|auto|status]]")?;
                    continue;
                }
            };
            if let Some(selected) = selected {
                approval::set_mode(&mode, selected)?;
            }
            ui.note(&format!(
                "[mode: {}; auto may allow actions without asking; shell is NOT sandboxed]",
                approval::mode(&mode)?
            ))?;
            if selected.is_some() && approval.load(Ordering::Acquire) {
                ui.note("[current approval still needs /approve or /deny; new mode applies to subsequent requests]")?;
            }
            continue;
        }
        if ["/approve", "/deny"].contains(&line.as_str())
            || (approval.load(Ordering::Acquire) && ["y", "n"].contains(&line.to_lowercase().as_str()))
        {
            if approval.swap(false, Ordering::AcqRel) {
                if let Some(input) = running.as_mut().and_then(|r| r.child.inner().stdin.as_mut()) {
                    input.write_all(if ["/approve", "y", "Y"].contains(&line.as_str()) { b"y\n" } else { b"n\n" })?;
                    input.flush()?;
                }
            } else {
                ui.note("[no pending approval]")?;
            }
        } else if line == "/exit" {
            exiting = true;
            followups.clear();
            ui.note("[exit requested; queued follow-ups discarded, current task will finish]")?;
        } else if exiting {
            ui.note("[exiting; only approvals are accepted]")?;
        } else if let Some(text) = line.strip_prefix("/queue ") {
            followups.push_back(text.to_owned().into());
            ui.note("[follow-up queued]")?;
        } else if line.starts_with('/') && !line.starts_with("/steer ") {
            ui.note("[invalid slash command arguments; type /help]")?;
        } else if running.is_some() || line.starts_with("/steer ") {
            steering::enqueue(&inbox, line.strip_prefix("/steer ").unwrap_or(&line))?;
            ui.note("[steering queued; applied at the next safe turn boundary]")?;
            if running.is_none() && followups.is_empty() {
                followups.push_back("Continue with the queued steering instructions.".into());
            }
        } else {
            followups.push_back(line.into());
        }
    }
    Ok(status)
}

#[cfg(test)]
mod composer_tests {
    use super::*;
    #[test]
    fn image_task_owns_private_temporary_files_until_finished_or_discarded() {
        let root = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(root.path()).unwrap();
        let args = vec!["-C".to_owned(), root.to_str().unwrap().to_owned()];
        let png = vec![137, 80, 78, 71, 13, 10, 26, 10];
        let task = image_task(String::new(), vec![png.clone()], &args).unwrap();
        let path = root.join(&task.images[0].1);
        assert_eq!(fs::read(&path).unwrap(), png);
        assert!(path.starts_with(root.join(".i/attachments")));
        drop(task);
        assert!(!path.exists());
        assert!(image_task("/approve".into(), vec![png], &args).is_err());
    }
}
