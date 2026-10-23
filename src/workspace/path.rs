use anyhow::Result;
use std::path::PathBuf;

pub fn validate(name: &str, allow_root: bool) -> Result<PathBuf> {
    crate::platform::workspace_path(name, allow_root)
}
