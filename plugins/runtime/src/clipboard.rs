use anyhow::{Context, Result, ensure};
use command_group::CommandGroup;
use std::{
    io::Read,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

// Called only for an explicit Ctrl-V, never on startup or while polling input.
pub(crate) fn read_png() -> Result<Vec<u8>> {
    let file = tempfile::NamedTempFile::new()?;
    let mut command = if cfg!(target_os = "macos") {
        let mut c = Command::new("osascript");
        c.args(["-e","on run argv\nset f to open for access POSIX file (item 1 of argv) with write permission\ntry\nset eof f to 0\nwrite (the clipboard as \u{ab}class PNGf\u{bb}) to f\nclose access f\non error m\nclose access f\nerror m\nend try\nend run"]);
        c.arg(file.path()).stdout(Stdio::null());
        c
    } else if cfg!(windows) {
        let mut c = Command::new("powershell.exe");
        c.args(["-NoProfile","-NonInteractive","-STA","-Command","Add-Type -AssemblyName System.Windows.Forms; Add-Type -AssemblyName System.Drawing; $img=[System.Windows.Forms.Clipboard]::GetImage(); if ($null -eq $img) { exit 1 }; try { $img.Save($env:HARNESS_CLIPBOARD_OUTPUT,[System.Drawing.Imaging.ImageFormat]::Png) } finally { $img.Dispose() }"]);
        c.env("HARNESS_CLIPBOARD_OUTPUT", file.path()).stdout(Stdio::null());
        c
    } else {
        let mut c = Command::new("wl-paste");
        c.args(["--no-newline", "--type", "image/png"]);
        c.stdout(Stdio::from(file.reopen()?));
        c
    };
    command.stdin(Stdio::null()).stderr(Stdio::null());
    let mut child = command
        .group_spawn()
        .context("Clipboard image unavailable (macOS osascript, Windows PowerShell or Linux wl-paste is required)")?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("Clipboard read timed out");
        }
        thread::sleep(Duration::from_millis(10));
    };
    ensure!(status.success(), "Clipboard does not contain a readable PNG image");
    ensure!(file.as_file().metadata()?.len() <= 8 * 1024 * 1024, "Clipboard image exceeds 8 MiB");
    let mut bytes = Vec::new();
    file.reopen()?.take(8 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 8 * 1024 * 1024 && bytes.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]),
        "Invalid clipboard PNG"
    );
    Ok(bytes)
}
