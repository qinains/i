use anyhow::{Context, Result, bail, ensure};
use std::{
    fs::{self, File, OpenOptions, TryLockError},
    path::{Path, PathBuf},
};

pub(crate) struct SessionLock {
    path: PathBuf,
    _guard: File,
}

impl SessionLock {
    pub(crate) fn acquire(path: &Path) -> Result<Self> {
        let path = if path.try_exists()? {
            dunce::canonicalize(path)?
        } else {
            let name = path.file_name().context("session path needs a file name")?;
            let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
            fs::create_dir_all(parent)?;
            dunce::canonicalize(parent)?.join(name)
        };
        let name = path.file_name().context("session path needs a file name")?;
        let parent = path.parent().context("session path needs a parent")?;
        ensure!(
            !parent.file_name().is_some_and(|name| name.eq_ignore_ascii_case(".i-session-locks")),
            "session path cannot point to the lock directory"
        );
        let directory = parent.join(".i-session-locks");
        fs::create_dir_all(&directory)?;
        // The session file is replaced at each checkpoint; the lock file must keep its identity.
        let guard =
            OpenOptions::new().read(true).write(true).create(true).truncate(false).open(directory.join(name))?;
        match guard.try_lock() {
            Ok(()) => Ok(Self { path, _guard: guard }),
            Err(TryLockError::WouldBlock) => bail!("session is in use by another process: {}", path.display()),
            Err(TryLockError::Error(error)) => Err(error).context("lock session"),
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}
