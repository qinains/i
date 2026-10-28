//! Cancelable, bounded native authorization child. Its private directory is uncommitted.
use super::{connections, paths::AppPaths};
use anyhow::{Context, Result, bail, ensure};
use command_group::{CommandGroup, GroupChild};
use serde_json::Value;
use std::{
    io::Read,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

pub(super) struct Job {
    pub id: String,
    pub phase: &'static str,
    child: Option<GroupChild>,
    output: mpsc::Receiver<(bool, std::io::Result<Vec<u8>>)>,
    stdout: Option<Vec<u8>>,
    stderr: Option<Vec<u8>>,
    deadline: Instant,
    pub directory: Option<tempfile::TempDir>,
}
impl Job {
    pub fn start(
        paths: &AppPaths,
        id: String,
        phase: &'static str,
        directory: tempfile::TempDir,
        account: Option<&str>,
    ) -> Result<Self> {
        let data = directory.path().to_path_buf();
        Self::launch(paths, id, phase, data, Some(directory), account)
    }
    pub fn catalog(paths: &AppPaths, id: String, data: std::path::PathBuf) -> Result<Self> {
        let root = dunce::canonicalize(&paths.data_dir)?;
        let data = dunce::canonicalize(data).context("Cannot open saved managed credential directory")?;
        let data =
            super::state::inside(&root, data.strip_prefix(&root).context("Credential directory escapes plugin data")?)?;
        Self::launch(paths, id, "models", data, None, None)
    }
    fn launch(
        paths: &AppPaths,
        id: String,
        phase: &'static str,
        data: std::path::PathBuf,
        directory: Option<tempfile::TempDir>,
        account: Option<&str>,
    ) -> Result<Self> {
        let exe = connections::checked_exe(paths, &id, true)?;
        let mut command = Command::new(exe);
        super::proxy::snapshot(&id)?.apply(&mut command);
        command.arg("--data-dir").arg(data).arg(phase);
        if let Some(account) = account {
            command.args(["--account", account]);
        }
        let mut child = command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).group_spawn()?;
        let (tx, output) = mpsc::channel();
        let stdout = child.inner().stdout.take().context("Missing plugin stdout")?;
        let stderr = child.inner().stderr.take().context("Missing plugin stderr")?;
        for (is_stdout, stream) in
            [(true, Box::new(stdout) as Box<dyn Read + Send>), (false, Box::new(stderr) as Box<dyn Read + Send>)]
        {
            let tx = tx.clone();
            thread::spawn(move || {
                let mut bytes = Vec::new();
                let result =
                    stream.take(if is_stdout { 2_000_001 } else { 16_001 }).read_to_end(&mut bytes).map(|_| bytes);
                let _ = tx.send((is_stdout, result));
            });
        }
        Ok(Self {
            id,
            phase,
            child: Some(child),
            output,
            stdout: None,
            stderr: None,
            deadline: Instant::now() + Duration::from_secs(if phase == "login" { 660 } else { 90 }),
            directory,
        })
    }
    pub fn poll(&mut self) -> Result<Option<Value>> {
        ensure!(Instant::now() < self.deadline, "Native {} timed out; pending credentials discarded", self.phase);
        while let Ok((stdout, bytes)) = self.output.try_recv() {
            let bytes = bytes?;
            ensure!(bytes.len() <= if stdout { 2_000_000 } else { 16_000 }, "Login plugin output exceeds limit");
            if stdout { self.stdout = Some(bytes) } else { self.stderr = Some(bytes) }
        }
        let status = self.child.as_mut().context("Missing login process")?.try_wait()?;
        if let Some(status) = status
            && let (Some(stdout), Some(stderr)) = (&self.stdout, &self.stderr)
        {
            self.child = None;
            if !status.success() {
                let message: String = String::from_utf8_lossy(stderr)
                    .chars()
                    .filter(|c| !c.is_control() || *c == '\n')
                    .take(2000)
                    .collect();
                bail!("{}: {}", self.id, message.trim());
            }
            return Ok(Some(serde_json::from_slice(stdout).context("Invalid login plugin result")?));
        }
        Ok(None)
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
