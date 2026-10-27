use anyhow::{Result, bail};
use std::path::{Path, PathBuf};
mod connections;
mod package;
#[cfg(unix)]
mod pty;
mod tls;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
}
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if std::env::var_os("I_NATIVE_CONSOLE_FIXTURE").is_some() {
        #[cfg(unix)]
        return pty::backend(&args);
    }
    if std::env::current_exe()?.file_stem().is_some_and(|name| name == "ant") {
        return connections::fake_ant(&args);
    }
    let binary = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| root().join("target/debug").join(if cfg!(windows) { "i.exe" } else { "i" }));
    match args.first().map(String::as_str) {
        Some("package-connections") => connections::package(&args[1..]),
        Some("check-connections") => connections::check(),
        Some("check-package") => {
            for pattern in &args[1..] {
                let files: Vec<_> = glob::glob(pattern)?.collect::<Result<_, _>>()?;
                anyhow::ensure!(!files.is_empty(), "No package matched {pattern}");
                for path in files {
                    package::check(&path)?;
                }
            }
            Ok(())
        }
        Some("check-tls") => tls::check(&binary, args.iter().any(|arg| arg == "--web")),
        #[cfg(unix)]
        Some("check-console") => pty::console(&binary),
        #[cfg(unix)]
        Some("check-setup") => pty::setup(&binary),
        #[cfg(unix)]
        Some("check-first-run") => pty::first_run(&binary),
        Some("binary-size") => {
            println!("Executable bytes: {}", std::fs::metadata(binary)?.len());
            Ok(())
        }
        _ => bail!(
            "Commands: package-connections, check-connections, check-package, check-tls, check-console, check-setup, check-first-run, binary-size"
        ),
    }
}

fn isolated(command: &mut std::process::Command, root: &Path) {
    command.env_clear();
    for key in ["PATH", "SystemRoot", "WINDIR", "TMPDIR", "TMP", "TEMP"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command.env("HOME", root).env("USERPROFILE", root).env("NO_PROXY", "127.0.0.1,localhost");
    for name in ["CONFIG", "DATA", "CACHE", "RUNTIME"] {
        command.env(format!("I_{name}_DIR"), root.join(name.to_ascii_lowercase()));
    }
}

fn run(command: &mut std::process::Command, input: &[u8]) -> Result<std::process::Output> {
    use std::io::Write;
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input)?;
    }
    let out = child.stdout.take().unwrap();
    let err = child.stderr.take().unwrap();
    let read = |mut pipe: Box<dyn std::io::Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = vec![];
            pipe.read_to_end(&mut bytes).map(|_| bytes)
        })
    };
    let stdout = read(Box::new(out));
    let stderr = read(Box::new(err));
    let start = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if start.elapsed() > Duration::from_secs(45) {
            let _ = child.kill();
            let _ = child.wait();
            bail!("Fixture process timed out");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(std::process::Output { status: child.wait()?, stdout: stdout.join().unwrap()?, stderr: stderr.join().unwrap()? })
}
