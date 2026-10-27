use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const LINE: &[u8] = b"fn main() { let counter = 42; }\n";
fn arguments(args: &[String]) -> Result<(Vec<PathBuf>, usize)> {
    let mut binaries = Vec::new();
    let mut runs = 3;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--runs" || arg.starts_with("--runs=") {
            let value = arg
                .strip_prefix("--runs=")
                .or_else(|| iter.next().map(String::as_str))
                .context("--runs requires a value")?;
            runs = value.parse().context("--runs must be 1..100")?;
        } else {
            ensure!(!arg.starts_with('-'), "unknown bench-search option: {arg}");
            binaries.push(PathBuf::from(arg));
        }
    }
    ensure!((1..=100).contains(&runs), "--runs must be 1..100");
    ensure!(!binaries.is_empty(), "bench-search requires one or more binaries");
    Ok((binaries, runs))
}
fn fixture(root: &Path) -> Result<(PathBuf, usize)> {
    let workspace = root.join("project");
    fs::create_dir(&workspace)?;
    let content = LINE.repeat(25_000);
    for index in 0..8 {
        fs::write(workspace.join(format!("{index}.txt")), &content)?;
    }
    Ok((workspace, content.len() * 8))
}
fn validate_output(stdout: &[u8]) -> Result<()> {
    let frame: Value = serde_json::from_slice(stdout).context("invalid search worker response")?;
    let output: Value =
        serde_json::from_str(frame["data"]["output"].as_str().context("search worker did not return output")?)?;
    ensure!(
        output["files_scanned"] == 8 && output["matches"] == json!([]),
        "search benchmark correctness check failed: {output}"
    );
    Ok(())
}
fn median(samples: &[f64]) -> f64 {
    let mut samples = samples.to_vec();
    samples.sort_by(f64::total_cmp);
    let middle = samples.len() / 2;
    if samples.len().is_multiple_of(2) { (samples[middle - 1] + samples[middle]) / 2.0 } else { samples[middle] }
}
pub(crate) fn run(args: &[String]) -> Result<()> {
    if args == ["--help"] {
        println!(
            "Small offline literal-search comparison, not a general throughput benchmark.\nUsage: i-repo-tool bench-search BINARY [BINARY ...] [--runs 1..100]"
        );
        return Ok(());
    }
    let (binaries, runs) = arguments(args)?;
    let temporary = tempfile::Builder::new().prefix("i-search-bench-").tempdir()?;
    let (workspace, bytes) = fixture(temporary.path())?;
    let mut frame = serde_json::to_vec(
        &json!({"id":1,"event":"tool","workspace":workspace,"data":{"name":"search","arguments":{"query":"absentneedle"}}}),
    )?;
    frame.push(b'\n');
    for binary in binaries {
        let executable =
            binary.canonicalize().with_context(|| format!("open benchmark binary {}", binary.display()))?;
        let mut samples = Vec::new();
        for _ in 0..runs {
            let start = Instant::now();
            let mut command = Command::new(&executable);
            command.args(["--runtime", "search"]).current_dir(temporary.path());
            for kind in ["CONFIG", "DATA", "CACHE", "RUNTIME"] {
                command.env(format!("I_{kind}_DIR"), temporary.path().join(kind.to_ascii_lowercase()));
            }
            let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
            let mut stdout = child.stdout.take().context("missing child stdout")?;
            let mut stderr = child.stderr.take().context("missing child stderr")?;
            let out = thread::spawn(move || {
                let mut bytes = Vec::new();
                stdout.read_to_end(&mut bytes).map(|_| bytes)
            });
            let err = thread::spawn(move || {
                let mut bytes = Vec::new();
                stderr.read_to_end(&mut bytes).map(|_| bytes)
            });
            child.stdin.take().context("missing child stdin")?.write_all(&frame)?;
            let status = loop {
                if let Some(status) = child.try_wait()? {
                    break status;
                }
                if start.elapsed() >= Duration::from_secs(30) {
                    let _ = child.kill();
                    let _ = child.wait();
                    anyhow::bail!("search benchmark timed out after 30 seconds");
                }
                thread::sleep(Duration::from_millis(1));
            };
            let stdout = out.join().map_err(|_| anyhow::anyhow!("stdout reader failed"))??;
            let stderr = err.join().map_err(|_| anyhow::anyhow!("stderr reader failed"))??;
            let elapsed = start.elapsed().as_secs_f64();
            ensure!(status.success(), "search benchmark child failed: {}", String::from_utf8_lossy(&stderr));
            validate_output(&stdout)?;
            samples.push(elapsed);
        }
        println!(
            "{}",
            json!({"binary":binary,"fixture_bytes":bytes,"seconds":samples,"median_seconds":median(&samples),"scope":"includes process startup and filesystem reads; first run may be cold"})
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keeps_binary_comparisons_and_bounded_run_count() {
        let (binaries, runs) = arguments(&["before".into(), "after".into(), "--runs".into(), "7".into()]).unwrap();
        assert_eq!(binaries, vec![PathBuf::from("before"), PathBuf::from("after")]);
        assert_eq!(runs, 7);
        for args in [vec![], vec!["--runs", "0", "binary"], vec!["binary", "--runs=101"], vec!["binary", "--runs"]] {
            assert!(arguments(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
    }
    #[test]
    fn fixture_and_statistics_match_the_original_measurement_scope() {
        let dir = tempfile::tempdir().unwrap();
        let (workspace, bytes) = fixture(dir.path()).unwrap();
        assert_eq!(bytes, LINE.len() * 25_000 * 8);
        assert_eq!(fs::read_dir(&workspace).unwrap().count(), 8);
        assert_eq!(fs::read(workspace.join("0.txt")).unwrap(), LINE.repeat(25_000));
        assert_eq!(median(&[9.0, 1.0, 3.0]), 3.0);
        assert_eq!(median(&[4.0, 2.0]), 3.0);
        let frame = json!({"data":{"output":json!({"files_scanned":8,"matches":[]}).to_string()}});
        assert!(validate_output(frame.to_string().as_bytes()).is_ok());
        assert!(validate_output(br#"{"data":{"output":"{\"files_scanned\":7,\"matches\":[]}"}}"#).is_err());
    }
}
