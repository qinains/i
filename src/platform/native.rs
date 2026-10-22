use super::*;
use anyhow::{Context, ensure};
use std::{
    fs,
    io::{self, Read, Write},
    process::Command,
};

pub struct NativeFs {
    root: PathBuf,
}
impl NativeFs {
    pub fn archive_target(&self, relative: &Path) -> Result<PathBuf> {
        let target = self.path(relative)?;
        let mut current = self.root.clone();
        for part in relative.components() {
            current.push(part);
            match fs::symlink_metadata(&current) {
                Ok(meta) => ensure!(!meta.file_type().is_symlink(), "archive output cannot contain symbolic links"),
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(target)
    }
    pub fn write_archive(&self, relative: &Path, bytes: &[u8]) -> Result<()> {
        let target = self.archive_target(relative)?;
        let parent = target.parent().context("missing archive parent")?;
        fs::create_dir_all(parent)?;
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        temp.write_all(bytes)?;
        temp.as_file().sync_all()?;
        // Replacing the directory entry never writes through an existing hard link.
        temp.persist(target)?;
        Ok(())
    }
    pub fn new(root: PathBuf) -> Result<Self> {
        let root = dunce::canonicalize(root)?;
        ensure!(root.is_dir(), "workspace must be a directory");
        Ok(Self { root })
    }
    pub fn path(&self, relative: &Path) -> Result<PathBuf> {
        ensure!(
            !relative.is_absolute()
                && relative
                    .components()
                    .all(|c| matches!(c, std::path::Component::Normal(_) | std::path::Component::CurDir)),
            "invalid workspace path"
        );
        let candidate = self.root.join(relative);
        let mut ancestor = candidate.as_path();
        loop {
            match fs::symlink_metadata(ancestor) {
                Ok(_) => break,
                Err(e) if e.kind() == io::ErrorKind::NotFound => ancestor = ancestor.parent().context("no parent")?,
                Err(e) => return Err(e.into()),
            }
        }
        ensure!(dunce::canonicalize(ancestor)?.starts_with(&self.root), "path escapes workspace");
        Ok(candidate)
    }
}
impl FileSystem for NativeFs {
    async fn read_file_limited(&self, path: &Path, limit: usize) -> Result<Vec<u8>> {
        let file = fs::File::open(self.path(path)?)?;
        ensure!(file.metadata()?.is_file(), "expected a regular file");
        let mut bytes = Vec::new();
        file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= limit, "file exceeds byte limit");
        Ok(bytes)
    }
    async fn write_file(&self, path: &Path, content: &[u8]) -> Result<()> {
        let target = self.path(path)?;
        fs::create_dir_all(target.parent().context("no parent")?)?;
        fs::write(target, content)?;
        Ok(())
    }
    async fn create_dir_all(&self, path: &Path) -> Result<()> {
        Ok(fs::create_dir_all(self.path(path)?)?)
    }
    async fn remove_file(&self, path: &Path) -> Result<()> {
        Ok(fs::remove_file(self.path(path)?)?)
    }
    async fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(self.path(path)?)? {
            let entry = entry?;
            ensure!(!entry.file_type()?.is_symlink(), "directory contains a symbolic link");
            entries.push(path.join(entry.file_name()));
        }
        entries.sort();
        Ok(entries)
    }
    async fn metadata(&self, path: &Path) -> Result<Option<FileMetadata>> {
        match fs::metadata(self.path(path)?) {
            Ok(m) => Ok(Some(FileMetadata { len: m.len(), is_file: m.is_file(), is_dir: m.is_dir() })),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    async fn canonicalize(&self, path: &Path) -> Result<PathBuf> {
        Ok(dunce::canonicalize(self.path(path)?)?.strip_prefix(&self.root)?.to_path_buf())
    }
}
pub struct NativeTerminal {
    pub readline: fn(&str) -> Result<String>,
}
impl Terminal for NativeTerminal {
    async fn read_line(&self, prompt: &str) -> Result<String> {
        (self.readline)(prompt)
    }
    fn write(&self, text: &str) -> Result<()> {
        io::stdout().write_all(text.as_bytes())?;
        Ok(())
    }
    async fn approve(&self, description: &str) -> Result<bool> {
        Ok((self.readline)(&format!("{description}\nAllow? [y/N] "))?.eq_ignore_ascii_case("y"))
    }
}
pub struct NativeProcess;
impl ProcessRunner for NativeProcess {
    async fn execute(&self, program: &str, args: &[&str]) -> Result<ProcessOutput> {
        let out = Command::new(program).args(args).output()?;
        Ok(ProcessOutput { status: out.status.code().unwrap_or(-1), stdout: out.stdout, stderr: out.stderr })
    }
    fn is_supported(&self) -> bool {
        true
    }
}
pub struct NativePlatform {
    pub fs: NativeFs,
    pub terminal: NativeTerminal,
    pub process: NativeProcess,
}
impl Platform for NativePlatform {
    type FileSystem = NativeFs;
    type Terminal = NativeTerminal;
    type Process = NativeProcess;
    fn fs(&self) -> &NativeFs {
        &self.fs
    }
    fn terminal(&self) -> &NativeTerminal {
        &self.terminal
    }
    fn process(&self) -> &NativeProcess {
        &self.process
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities::NATIVE
    }
}
