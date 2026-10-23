use super::*;
use sha2::{Digest, Sha256};
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn default_dir() -> Result<PathBuf> {
    if cfg!(windows) {
        Ok(PathBuf::from(env::var_os("LOCALAPPDATA").context("LOCALAPPDATA is missing")?).join("Programs/i/bin"))
    } else {
        Ok(PathBuf::from(env::var_os("HOME").context("HOME is missing")?).join(".local/bin"))
    }
}
pub(super) fn copy(source: &Path, dir: &Path, replace: bool, expected: Option<&str>) -> Result<PathBuf> {
    ensure!(dir.is_absolute(), "installation directory must be absolute");
    let bytes = state::read_bounded(source, 64 * 1024 * 1024)?;
    let digest = hash(&bytes);
    if let Some(expected) = expected {
        ensure!(
            expected.len() == 64
                && expected.bytes().all(|b| b.is_ascii_hexdigit())
                && expected.eq_ignore_ascii_case(&digest),
            "executable SHA256 mismatch"
        );
    }
    std::fs::create_dir_all(dir)?;
    let dir = dunce::canonicalize(dir)?;
    let _lock = state::Lock::acquire(&dir.join(".i-install.lock"))?;
    let target = dir.join(format!("i{}", env::consts::EXE_SUFFIX));
    regular(&target)?;
    let before = if target.try_exists()? { Some(state::read_bounded(&target, 64 * 1024 * 1024)?) } else { None };
    if before.as_deref() == Some(bytes.as_slice()) {
        return Ok(target);
    }
    if before.is_some() {
        ensure!(
            replace || confirm(&format!("Replace {}? A backup will be kept.", target.display()))?,
            "existing executable preserved; use --replace to authorize replacement"
        );
        let saved = backup(&target)?;
        std::fs::set_permissions(&saved, std::fs::metadata(&target)?.permissions())?;
        eprintln!("Executable backup: {}", saved.display());
    }
    let mut temporary = tempfile::NamedTempFile::new_in(&dir)?;
    temporary.write_all(&bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temporary.as_file().set_permissions(std::fs::Permissions::from_mode(0o755))?;
    }
    temporary.as_file().sync_all()?;
    ensure!(
        hash(&state::read_bounded(temporary.path(), 64 * 1024 * 1024)?).as_bytes() == digest.as_bytes(),
        "copied executable verification failed"
    );
    regular(&target)?;
    if let Some(before) = before {
        ensure!(state::read_bounded(&target, 64 * 1024 * 1024)? == before, "installed executable changed concurrently");
        temporary.persist(&target)?;
    } else {
        temporary.persist_noclobber(&target)?;
    }
    #[cfg(unix)]
    std::fs::File::open(&dir)?.sync_all()?;
    Ok(target)
}
pub(super) fn run(args: &[String]) -> Result<i32> {
    ensure!(
        crate::HOSTED.load(std::sync::atomic::Ordering::Relaxed),
        "install requires the bundled i executable, not i-plugins"
    );
    let opts = options(args, &["--bin-dir", "--sha256"], &["--replace", "--add-to-path"])?;
    let source = env::current_exe()?;
    let dir = opts.get("--bin-dir").map(PathBuf::from).map(Ok).unwrap_or_else(default_dir)?;
    let checksums = source.parent().and_then(Path::parent).context("missing executable directory")?.join("SHA256SUMS");
    let mut expected = opts.get("--sha256").cloned();
    if expected.is_none() && checksums.try_exists()? {
        let text = String::from_utf8(state::read_bounded(&checksums, 1_000_000)?)?;
        let name = format!("bin/i{}", env::consts::EXE_SUFFIX);
        expected = Some(
            text.lines()
                .find_map(|line| {
                    let (hash, path) = line.split_once("  ")?;
                    (path == name).then(|| hash.to_owned())
                })
                .context("package checksum does not include executable")?,
        );
    }
    if expected.is_none() {
        eprintln!("Installing the current executable; no independently trusted download checksum supplied.");
    }
    let target = copy(&source, &dir, opts.contains_key("--replace"), expected.as_deref())?;
    println!("Installed: {}\nSHA256: {}", target.display(), hash(&state::read_bounded(&target, 64 * 1024 * 1024)?));
    let in_path = env::var_os("PATH").is_some_and(|v| {
        env::split_paths(&v).any(|p| dunce::canonicalize(p).ok() == target.parent().map(Path::to_path_buf))
    });
    let first = env::var_os("PATH")
        .and_then(|v| env::split_paths(&v).map(|p| p.join(target.file_name().unwrap())).find(|p| p.is_file()));
    let shadowed = first.as_ref().is_some_and(|p| dunce::canonicalize(p).ok().as_ref() != Some(&target));
    if shadowed {
        eprintln!(
            "Another i currently precedes this installation on PATH: {}. Use the installed absolute path until your PATH is updated.",
            first.unwrap().display()
        );
    }
    if !in_path || shadowed {
        if opts.contains_key("--add-to-path") {
            add_path(target.parent().unwrap())?;
        } else {
            eprintln!(
                "Installation is missing from or shadowed on PATH. Rerun with --add-to-path or prioritize {} manually.",
                target.parent().unwrap().display()
            );
        }
    }
    println!(
        "Next: {} setup\nNo personal configuration or sessions were replaced. Start a new login shell/session after PATH changes.",
        target.display()
    );
    Ok(0)
}
#[cfg(not(windows))]
fn add_path(dir: &Path) -> Result<()> {
    let home = PathBuf::from(env::var_os("HOME").context("HOME missing")?);
    let shell = env::var("SHELL").unwrap_or_default();
    let name = Path::new(&shell).file_name().and_then(|s| s.to_str()).unwrap_or("");
    ensure!(
        ["", "sh", "bash", "zsh", "dash", "ksh"].contains(&name),
        "automatic PATH editing does not support this shell; add the installation directory manually"
    );
    let file = home.join(if shell.ends_with("zsh") {
        ".zprofile"
    } else if shell.ends_with("bash") && home.join(".bash_profile").is_file() {
        ".bash_profile"
    } else if shell.ends_with("bash") && home.join(".bash_login").is_file() {
        ".bash_login"
    } else {
        ".profile"
    });
    regular(&file)?;
    let before = if file.try_exists()? { state::read_bounded(&file, 1_000_000)? } else { vec![] };
    let mut text = String::from_utf8(before.clone()).context("shell profile is not UTF-8")?;
    let dir = dir.to_str().context("PATH directory is not UTF-8")?;
    ensure!(!dir.chars().any(char::is_control), "invalid PATH directory");
    let line = format!("export PATH='{}':\"$PATH\"", dir.replace('\'', "'\"'\"'"));
    if text.lines().any(|s| s == line) {
        return Ok(());
    }
    if file.try_exists()? {
        eprintln!("Shell profile backup: {}", backup(&file)?.display());
    }
    text.push_str(&format!("\n# i user installation\n{line}\n"));
    let mut tmp = tempfile::NamedTempFile::new_in(&home)?;
    tmp.write_all(text.as_bytes())?;
    tmp.as_file().sync_all()?;
    if file.try_exists()? {
        ensure!(state::read_bounded(&file, 1_000_000)? == before, "shell profile changed concurrently");
        tmp.persist(&file)?;
    } else {
        tmp.persist_noclobber(&file)?;
    }
    Ok(())
}
#[cfg(windows)]
fn add_path(dir: &Path) -> Result<()> {
    let script = "$d=$env:I_INSTALL_BIN;$p=[Environment]::GetEnvironmentVariable('Path','User');if (($p -split ';') -notcontains $d) {[Environment]::SetEnvironmentVariable('Path',($d+';'+$p),'User')}";
    ensure!(
        std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("I_INSTALL_BIN", dir)
            .status()?
            .success(),
        "user PATH update failed"
    );
    Ok(())
}
