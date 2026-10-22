use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[cfg(not(target_arch = "wasm32"))]
pub mod native;

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
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub len: u64,
    pub is_file: bool,
    pub is_dir: bool,
}
pub trait FileSystem {
    async fn read_file_limited(&self, path: &Path, limit: usize) -> Result<Vec<u8>>;
    async fn read_file(&self, path: &Path) -> Result<Vec<u8>> {
        self.read_file_limited(path, 64 * 1024 * 1024).await
    }
    async fn write_file(&self, path: &Path, content: &[u8]) -> Result<()>;
    async fn create_dir_all(&self, path: &Path) -> Result<()>;
    async fn remove_file(&self, path: &Path) -> Result<()>;
    async fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>>;
    async fn metadata(&self, path: &Path) -> Result<Option<FileMetadata>>;
    async fn canonicalize(&self, path: &Path) -> Result<PathBuf>;
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
}
pub trait ProcessRunner {
    async fn execute(&self, program: &str, args: &[&str]) -> Result<ProcessOutput>;
    fn is_supported(&self) -> bool;
}
