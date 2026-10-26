use anyhow::{Result, ensure};
use std::{io::Read, path::Path};

fn check(path: &Path) -> Result<()> {
    let file = std::fs::File::open(path)?;
    ensure!(file.metadata()?.len() <= i_wasm_package::PACKAGE_LIMIT as u64, "plugin package exceeds 10 MiB");
    let mut bytes = Vec::new();
    file.take(i_wasm_package::PACKAGE_LIMIT as u64 + 1).read_to_end(&mut bytes)?;
    i_wasm_package::inspect_package(&bytes)?;
    Ok(())
}

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    ensure!(args.len() == 1, "Usage: i-plugin-check INPUT.hplugin.zip");
    check(Path::new(&args[0]))
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_container_and_oversized_input() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("plugin.zip");
        std::fs::write(&path, b"not a ZIP").unwrap();
        assert!(check(&path).is_err());
        std::fs::File::create(&path).unwrap().set_len(i_wasm_package::PACKAGE_LIMIT as u64 + 1).unwrap();
        assert!(check(&path).unwrap_err().to_string().contains("10 MiB"));
    }
}
