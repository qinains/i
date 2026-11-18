use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

pub(crate) fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>> {
    ensure!(fs::metadata(path)?.is_file(), "expected a regular file: {}", path.display());
    let mut bytes = Vec::new();
    File::open(path)?.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= limit, "file too large: {}", path.display());
    Ok(bytes)
}
pub(crate) fn inside(root: &Path, relative: &Path) -> Result<PathBuf> {
    ensure!(
        !relative.is_absolute() && relative.components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir)),
        "path must be workspace-relative without '..'"
    );
    let root = dunce::canonicalize(root)?;
    let path = root.join(relative);
    let mut ancestor = path.as_path();
    loop {
        match fs::symlink_metadata(ancestor) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                ancestor = ancestor.parent().context("missing ancestor")?;
            }
            Err(error) => return Err(error.into()),
        }
    }
    ensure!(dunce::canonicalize(ancestor)?.starts_with(&root), "path escapes workspace");
    Ok(path)
}
// The host supplies the workspace-specific state root, independent of project files.
pub(crate) fn workspace_state(root: &Path, relative: &str) -> Result<PathBuf> {
    inside(root, Path::new(relative))
}
pub(crate) struct Lock(File);
impl Lock {
    pub(crate) fn acquire(path: &Path) -> Result<Self> {
        ensure!(!path.is_symlink(), "lock cannot be a symlink");
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(path)?;
        fs2::FileExt::lock_exclusive(&file)?;
        Ok(Self(file))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.0);
    }
}
pub(crate) fn atomic(path: &Path, value: &Value) -> Result<()> {
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().context("missing parent")?)?;
    serde_json::to_writer_pretty(&mut temp, value)?;
    temp.flush()?;
    temp.as_file().sync_all()?;
    persist_atomic(temp, path).with_context(|| format!("replace state file: {}", path.display()))?;
    Ok(())
}

#[cfg(not(windows))]
fn persist_atomic(temp: tempfile::NamedTempFile, path: &Path) -> std::io::Result<()> {
    temp.persist(path).map(|_| ()).map_err(|error| error.error)
}

#[cfg(windows)]
fn persist_atomic(temp: tempfile::NamedTempFile, path: &Path) -> std::io::Result<()> {
    use std::time::{Duration, Instant};
    let deadline = Instant::now() + Duration::from_secs(1);
    let mut delay = Duration::from_millis(10);
    persist_on_contention(temp, path, || {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return false;
        }
        std::thread::sleep(delay.min(remaining));
        delay = (delay * 2).min(Duration::from_millis(80));
        true
    })
}

#[cfg(windows)]
fn persist_on_contention(
    mut temp: tempfile::NamedTempFile,
    path: &Path,
    mut wait: impl FnMut() -> bool,
) -> std::io::Result<()> {
    // A foreign handle may temporarily deny replacement. Retry only the rename
    // of the same synced file; do not delete the old file or replay the update.
    const ACCESS_DENIED: i32 = 5;
    const SHARING_VIOLATION: i32 = 32;
    const LOCK_VIOLATION: i32 = 33;
    loop {
        match temp.persist(path) {
            Ok(_) => return Ok(()),
            Err(error)
                if matches!(error.error.raw_os_error(), Some(ACCESS_DENIED | SHARING_VIOLATION | LOCK_VIOLATION))
                    && wait() =>
            {
                temp = error.file;
            }
            Err(error) => return Err(error.error),
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::os::windows::fs::OpenOptionsExt;

    #[test]
    fn state_replacement_keeps_old_data_until_the_sharing_conflict_is_gone() {
        const SHARE_READ: u32 = 1;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory.json");
        fs::write(&path, "old").unwrap();
        let mut reader = Some(OpenOptions::new().read(true).share_mode(SHARE_READ).open(&path).unwrap());
        let staged = || {
            let mut temp = tempfile::NamedTempFile::new_in(dir.path()).unwrap();
            temp.write_all(b"new").unwrap();
            temp.as_file().sync_all().unwrap();
            temp
        };

        let mut blocked = false;
        assert!(
            persist_on_contention(staged(), &path, || {
                blocked = true;
                false
            })
            .is_err()
        );
        assert!(blocked, "expected a recognized Windows sharing error");
        assert_eq!(fs::read(&path).unwrap(), b"old");

        let mut attempts = 0;
        persist_on_contention(staged(), &path, || {
            assert_eq!(fs::read(&path).unwrap(), b"old");
            drop(reader.take());
            attempts += 1;
            std::thread::sleep(std::time::Duration::from_millis(10));
            attempts < 100
        })
        .unwrap();
        assert!(reader.is_none());
        assert!(attempts > 0);
        assert_eq!(fs::read(&path).unwrap(), b"new");
    }
}
