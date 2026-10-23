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
    temp.persist(path)?;
    Ok(())
}
