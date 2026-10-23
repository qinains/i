use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[cfg(not(target_arch = "wasm32"))]
pub mod native;
#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
pub mod wasm;

#[derive(Clone, Copy)]
pub struct Capabilities {
    pub files: bool,
    pub processes: bool,
    pub download: bool,
}
impl Capabilities {
    pub const BROWSER: Self = Self { files: true, processes: false, download: true };
    pub const NATIVE: Self = Self { files: true, processes: true, download: true };
}
pub trait Platform {
    type FileSystem: FileSystem;
    type Terminal: Terminal;
    type Process: ProcessRunner;
    fn fs(&self) -> &Self::FileSystem;
    fn terminal(&self) -> &Self::Terminal;
    fn process(&self) -> &Self::Process;
    fn capabilities(&self) -> Capabilities;
    fn root(&self) -> &Path;
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub len: u64,
    pub is_file: bool,
    pub is_dir: bool,
}
pub trait FileSystem {
    fn resolve_path(&self, name: &str, allow_root: bool) -> Result<PathBuf> {
        workspace_path(name, allow_root)
    }
    async fn read_file_limited(&self, path: &Path, limit: usize) -> Result<Vec<u8>>;
    async fn read_file(&self, path: &Path) -> Result<Vec<u8>> {
        self.read_file_limited(path, 64 * 1024 * 1024).await
    }
    async fn write_file(&self, path: &Path, content: &[u8]) -> Result<()>;
    async fn create_dir_all(&self, path: &Path) -> Result<()>;
    async fn remove_file(&self, path: &Path) -> Result<()>;
    async fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>>;
    async fn metadata(&self, path: &Path) -> Result<Option<FileMetadata>>;
    async fn symlink_metadata(&self, path: &Path) -> Result<Option<FileMetadata>> {
        self.metadata(path).await
    }
    async fn exists(&self, path: &Path) -> Result<bool> {
        Ok(self.metadata(path).await?.is_some())
    }
    async fn canonicalize(&self, path: &Path) -> Result<PathBuf>;
}
pub fn workspace_path(name: &str, allow_root: bool) -> Result<PathBuf> {
    anyhow::ensure!(allow_root || !name.is_empty(), "path cannot be empty");
    anyhow::ensure!(!name.starts_with('/') && !name.contains(['\\', '\0', ':']), "path must be workspace-relative");
    anyhow::ensure!(!name.split('/').any(|part| part == ".."), "path cannot contain '..'");
    Ok(name.split('/').filter(|s| !s.is_empty() && *s != ".").collect())
}
pub trait Terminal {
    async fn read_line(&self, prompt: &str) -> Result<String>;
    fn write(&self, text: &str) -> Result<()>;
    async fn approve(&self, description: &str) -> Result<bool>;
}
pub struct ProcessOutput {
    pub status: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub timed_out: bool,
}
pub trait ProcessRunner {
    async fn execute(&self, program: &str, args: &[&str]) -> Result<ProcessOutput>;
    async fn execute_shell(
        &self,
        command: &str,
        working_dir: &Path,
        timeout: Duration,
        io_limit: usize,
    ) -> Result<ProcessOutput>;
    fn is_supported(&self) -> bool;
}
