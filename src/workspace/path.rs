use anyhow::{Result, ensure};
use std::path::PathBuf;

pub fn validate(name: &str, allow_root: bool) -> Result<PathBuf> {
    ensure!(allow_root || !name.is_empty(), "path cannot be empty");
    ensure!(!name.starts_with('/') && !name.contains(['\\', '\0', ':']), "path must be workspace-relative");
    ensure!(!name.split('/').any(|part| part == ".."), "path cannot contain '..'");
    let path: PathBuf = name.split('/').filter(|s| !s.is_empty() && *s != ".").collect();
    ensure!(allow_root || !path.as_os_str().is_empty(), "path cannot be workspace root");
    Ok(path)
}
