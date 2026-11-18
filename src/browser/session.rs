use anyhow::{Result, ensure};
pub fn workspace_id(id: &str) -> Result<()> {
    ensure!(id != "temp-default", "invalid workspace ID");
    let id = id.strip_prefix("temp-").unwrap_or(id);
    let bytes = id.as_bytes();
    ensure!(
        id == "default"
            || (bytes.len() == 36
                && bytes.iter().enumerate().all(|(i, b)| {
                    if [8, 13, 18, 23].contains(&i) {
                        *b == b'-'
                    } else {
                        b.is_ascii_digit() || (b'a'..=b'f').contains(b)
                    }
                })
                && bytes[14] == b'4'
                && b"89ab".contains(&bytes[19])),
        "invalid workspace ID"
    );
    Ok(())
}
pub fn browser_path(name: &str, allow_root: bool) -> Result<std::path::PathBuf> {
    use crate::workspace::path::validate;
    if name == "/tmp" || name.starts_with("/tmp/") {
        let path = validate(name.strip_prefix("/tmp").unwrap().trim_start_matches('/'), true)?;
        return Ok(std::path::Path::new("/tmp").join(path));
    }
    if name == "/workspace" || name.starts_with("/workspace/") {
        return validate(name.strip_prefix("/workspace").unwrap().trim_start_matches('/'), allow_root);
    }
    validate(name, allow_root)
}
