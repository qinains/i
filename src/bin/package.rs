//! Packaging utility for creating distribution archives.
//! Build with: cargo build --features packaging --bin i-package

use anyhow::{Context, Result, bail};
use clap::Parser;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Parser)]
#[command(name = "i-package", about = "Package i for distribution")]
struct Args {
    /// Build profile
    #[arg(long, default_value = "min-size", value_parser = ["release", "min-size"])]
    profile: String,

    /// Skip cargo build (use existing binaries)
    #[arg(long)]
    skip_build: bool,

    /// Extra AArch64 min-size LLVM outlining (requires build)
    #[arg(long)]
    outline: bool,
}

const README: &str = r#"i / lninl — native CLI release build

Contents: bin/i (core + optional runtime), config.example.json, start.sh, logo.svg,
BUILD.json and SHA256SUMS. No Python/Node/Rust installation is needed to run
these binaries. The platform's system libraries are still required.

Quick start:
  Run ./bin/i in a terminal (Windows PowerShell: .\bin\i.exe).
  First launch offers optional user-local installation and PATH setup, then
  configures the model if needed and opens the session. Declining installation
  is remembered. No separate install/setup/start commands are required.
  Existing different executables require confirmation; backups are kept.
  After PATH changes, open a new login shell/session and run i.
  Manual installation remains available: ./bin/i install --add-to-path.
The launcher uses your global settings, not config.example.json. Setup never
probes a model or makes paid API requests. Unknown context capacity is NOT guessed:
fresh setup explicitly disables budget/automatic compaction until --context-window
is supplied; existing budget and advanced fields are preserved.

Key entry is hidden; OS storage uses macOS Keychain, Linux secret-tool/Secret Service
(optional helper), or Windows current-user DPAPI. --key-env NAME uses an environment
variable instead. --key-stdin accepts a pipe, never a command-line key argument.
No plaintext key is saved to JSON; OS storage may require unlocking/authorization.
OS credential integration is implemented but not exercised against personal stores
by the offline tests; Linux/Windows execution is not locally verified.

Use bin/i --runtime init to create missing ~/.i/config defaults without overwriting.
New bundled configs use a portable builtin.json descriptor: built-in workers resolve
to the running installed executable, including after relocation. Custom bindings
remain explicit and unchanged. Existing desktop.json paths are NOT silently rewritten:
i setup --migrate-builtins --confirm migrates only recognizable generated manifests,
backing up API config and retaining the original desktop manifest. A custom manifest
needs manual review. /exit finishes a session; Ctrl-C aborts an active task.
The installer copies a locally supplied executable; this is not a release downloader.
SHA256SUMS detects corruption, not publisher authenticity. Verify download provenance.

Networking uses synchronous HTTP with certificate/hostname verification enabled.
The core uses Rustls with Mozilla roots; the optional web worker uses system TLS
(OpenSSL and system CA certificates on Linux). No external curl/Python process
handles networking.
API config supports proxy (HTTP CONNECT, SOCKS5, SOCKS5h), with an empty string
forcing direct connections; omit it to inherit proxy environment variables.
Web tools disable proxies and validate the actual resolved destination addresses.
Transient model failures retry up to 5 times (network_retries: 0..8), with bounded
backoff and a 180-second overall network deadline. A lost response may be billed
again. Local tools are never replayed. Configured session checkpoints survive
exhaustion; no partial response is executed. Ctrl-C aborts the console task.

Approval is a replaceable approval worker: /mode ask|auto switches console mode,
/mode shows status. Auto is NOT a sandbox;
a custom policy can still ask or deny. Pending questions need an explicit answer.
Bindings can replace approval with an absolute worker argv; console workers receive
--mode-file PATH. Invalid decisions or worker failures never fall back to allowing.

Provider workers own vendor wire formats: openai, deepseek, anthropic, responses.
Core HTTP/TLS, credentials, origin checks, retries and tool gates remain generic.
Use protocol=responses with a standard manifest, not the legacy responses hook.
A core-only build requires an external provider worker or sibling i-plugins;
this bundled binary includes one. Provider failures never fall back to another.
DeepSeek Files mode is opt-in: API config provider_operations=true plus provider.json
images=files, account_scope, expires_after_seconds (default 3600; null means permanent).
Core executes bounded same-origin multipart POSTs without upload retries. Original
image snapshots remain in sessions. Pending/expired IDs fail closed; no Base64 fallback.
Use bin/i --runtime provider-files -C PROJECT to inspect local mappings; --forget KEY
--confirm removes only a local mapping, NOT the remote file. Pending state additionally
requires --ack-unknown after checking remote files; retrying may create duplicates.
Remote Files management uses bin/i --config API_CONFIG -C PROJECT --provider-action
'{"op":"files.list","limit":100}' (also files.get/files.delete with file_id).
No model or session is used. Ordinary DELETE always asks, even with --yolo /
--dangerously-skip-permissions (old --yes removed), and is never retried.
files.reconcile uses key, file_id and confirm=true to recover a local mapping after
GET metadata matches filename/size/expiry; metadata is NOT proof of image content.
Confirmed deletions keep local tombstones to prevent silent re-upload in that scope.
Files also supports OpenAI Responses (purpose=vision) and Anthropic's compatible
files-api-2025-04-14 beta (explicit expires_after_seconds:null; no server TTL).
Optional auto_refresh.enabled replaces known expiring IDs; ambiguous uploads never replay.
Optional auto_gc.enabled collects only superseded confirmed-owned IDs after a grace
period, not the whole account or current mappings. It requires separate API-config
provider_cleanup:true; yolo does not grant this capability. No background daemon.
Inspect with --provider-action '{"op":"files.gc"}'; add "dry_run":false to execute
under the explicit cleanup grants. External references cannot be discovered: only
use auto_gc for an exclusively managed file pool. Unknown deletions never replay.
GC reports classify waits, references, unknown outcomes and invalid records.
--provider-action '{"op":"files.gc-inspect","key":"RETIRED_KEY"}' performs a
read-only metadata GET without resetting cleanup state. GET existence does not
prove an earlier DELETE was cancelled; 404 is not silently treated as success.

Compaction is a separate, replaceable compact worker. Standard/full profiles
include it; budget-only profiles do not compress. Configure the built-in policy
in config_dir/compact.json. Host-specific bindings.json can replace compact,
budget, memory and other selected workers with an absolute exec argv and optional
absolute cwd. Replacement failure never falls back silently; core history,
budget, tool-batch and checkpoint guards still apply. Portable builtin descriptors
resolve bindings on each launch; legacy/custom executable manifests remain explicit.

Both .tar.xz (smaller download) and .tar.gz (compatibility) contain the same files.
Archive compression does not reduce the extracted executable size. Unpack with
an xz/gzip-capable archive tool; no decompressor is needed to run the executable.

Build profiles:
  release: standard Cargo release optimization.
  min-size: opt-level=z, fat LTO, one codegen unit, symbols stripped,
            panic=abort, no incremental compilation. Agent/plugin modules stay enabled.
  Optional --outline: additional AArch64 min-size machine outlining; flags are
            recorded in BUILD.json. Only macOS arm64 was locally exercised.
Full regex Unicode syntax and performance backends remain enabled by the shared
workspace search tool. Globset's upstream Rust sources remain unmodified.
Project MIT and third-party copyright/license texts are included separately.
The core uses Rustls with Mozilla roots; the optional ureq web worker uses
platform-native TLS. Their trust stores can differ.
BPE vocabularies are not bundled. Budgeting uses returned usage where valid,
with conservative byte/image estimates for unmeasured history and new content.
min-size trades compilation time/debuggability (and possibly execution speed)
for file size. Abort does not unwind/guarantee cleanup after an unexpected panic;
ordinary Result errors and timeout cleanup retain their normal paths.

The console has a no-new-crate UTF-8 editor for Chinese/ASCII, horizontal scrolling,
bracketed paste and in-memory history. Background output preserves the draft.
Unsupported glyphs (including complex emoji/combining marks) display as ASCII
escapes in the editable row; submitted text is preserved. Editing is scalar-based,
not a full Unicode grapheme engine. Piped/non-TTY input keeps line-based behavior.
Normal exits/errors/Ctrl-C restore terminal modes. SIGKILL/panic=abort cannot
run cleanup: use the terminal's reset action if an abnormal kill leaves raw mode.

Release build does not mean public-release readiness. Windows terminal execution
has not been locally verified. Binaries are not Developer-ID signed or
notarized for macOS distribution (toolchain ad-hoc signatures may be present).
Only the recorded native target was built; this is not a universal binary.
Preserve LICENSE.txt, THIRD_PARTY_LICENSES.txt and upstream notices when redistributing.

Sizes exclude personal configuration, sessions, metrics, caches, and build
artifacts. SHA256SUMS covers every other packaged file, including BUILD.json.
"#;

const LAUNCHER: &str = r#"#!/bin/sh
# Launch packaged Rust programs; no model or tool logic lives in this wrapper.
set -eu
if [ "$#" -gt 1 ]; then
    printf 'Usage: ./start.sh [workspace]\n' >&2
    exit 2
fi
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
exec "$here/bin/i" start --dir="${1:-$PWD}"
"#;

fn main() -> Result<()> {
    let args = Args::parse();

    // Validate outline flag
    if args.outline && (args.skip_build || args.profile != "min-size") {
        bail!("--outline requires an AArch64 min-size build, without --skip-build");
    }

    // Detect host target
    let host = detect_host()?;
    println!("==> Detected host: {}", host);

    // Validate outline for AArch64
    if args.outline && !host.starts_with("aarch64-") {
        bail!("--outline requires AArch64 target (got: {})", host);
    }

    // Read version from Cargo.toml
    let version = read_version()?;
    println!("==> Version: {}", version);

    // Get project root
    let root = project_root()?;
    println!("==> Project root: {}", root.display());

    // Build if needed
    if !args.skip_build {
        println!("==> Building with cargo...");
        cargo_build(&root, &args)?;
    }

    // Locate binaries
    let binaries_dir = if args.skip_build {
        root.join("target").join(&args.profile)
    } else {
        root.join("target").join(&host).join(&args.profile)
    };

    let suffix = if host.contains("windows") { ".exe" } else { "" };
    let binary_source = binaries_dir.join(format!("i{}", suffix));

    if !binary_source.exists() {
        bail!("Binary not found at {}", binary_source.display());
    }

    // Create package directory
    let package_name = format!("i-{}-{}-{}", version, host, args.profile);
    let dist_dir = root.join("dist");
    fs::create_dir_all(&dist_dir)?;

    let temp_dir = tempfile::Builder::new().prefix(".package-").tempdir_in(&dist_dir)?;

    let package_dir = temp_dir.path().join(&package_name);
    let bin_dir = package_dir.join("bin");
    fs::create_dir_all(&bin_dir)?;

    // Copy binary
    println!("==> Copying binary...");
    let binary_dest = bin_dir.join(format!("i{}", suffix));
    fs::copy(&binary_source, &binary_dest)?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&binary_dest)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&binary_dest, perms)?;
    }

    let binary_size = fs::metadata(&binary_dest)?.len();

    // Verify binary works
    println!("==> Verifying binary...");
    let verify_result = Command::new(&binary_dest).args(["--runtime", "logo", "--plain"]).output();

    if verify_result.is_err() {
        bail!("Binary failed to execute");
    }

    // Generate config.example.json
    println!("==> Generating config.example.json...");
    generate_config_example(&root, &package_dir)?;

    // Write README.txt
    fs::write(package_dir.join("README.txt"), README)?;

    // Write start.sh
    let start_sh = package_dir.join("start.sh");
    fs::write(&start_sh, LAUNCHER)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&start_sh)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&start_sh, perms)?;
    }

    // Copy logo and license
    fs::copy(root.join("assets/lninl.svg"), package_dir.join("logo.svg"))?;
    fs::copy(root.join("vendor/globset/LICENSE-MIT"), package_dir.join("LICENSE-globset.txt"))?;
    fs::copy(root.join("LICENSE"), package_dir.join("LICENSE.txt"))?;
    fs::copy(root.join("THIRD_PARTY_LICENSES.txt"), package_dir.join("THIRD_PARTY_LICENSES.txt"))?;

    // Generate BUILD.json
    println!("==> Generating BUILD.json...");
    generate_build_json(&package_dir, &version, &host, &args.profile, args.outline && !args.skip_build, binary_size)?;

    // Generate SHA256SUMS
    println!("==> Generating SHA256SUMS...");
    generate_sha256sums(&package_dir)?;

    // Calculate unpacked size
    let unpacked_size = calculate_dir_size(&package_dir)?;

    // Create archives (platform-specific format)
    println!("==> Creating archive...");

    if host.contains("windows") {
        create_zip_archive(temp_dir.path(), &dist_dir, &package_name, binary_size, unpacked_size)?;
    } else {
        create_tar_archive(temp_dir.path(), &dist_dir, &package_name, binary_size, unpacked_size)?;
    }

    println!("==> Done!");
    Ok(())
}

fn detect_host() -> Result<String> {
    let output = Command::new("rustc").arg("-vV").output().context("Failed to run rustc")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(host) = line.strip_prefix("host: ") {
            return Ok(host.trim().to_string());
        }
    }

    bail!("Failed to detect rustc host target")
}

fn project_root() -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    // Navigate from target/{profile}/i-package to project root
    let mut path = exe.parent().context("No parent")?;
    while !path.join("Cargo.toml").exists() {
        path = path.parent().context("No parent")?;
    }
    Ok(path.to_path_buf())
}

fn read_version() -> Result<String> {
    let root = project_root()?;
    let cargo_toml = fs::read_to_string(root.join("Cargo.toml"))?;

    for line in cargo_toml.lines() {
        if let Some(rest) = line.strip_prefix("version = ")
            && let Some(version) = rest.strip_prefix('"').and_then(|s| s.split('"').next())
        {
            return Ok(version.to_string());
        }
    }

    bail!("Failed to extract version from Cargo.toml")
}

fn cargo_build(root: &Path, args: &Args) -> Result<()> {
    let host = detect_host()?;

    let mut cmd = Command::new("cargo");

    if args.outline {
        cmd.arg("rustc");
    } else {
        cmd.arg("build");
    }

    cmd.args([
        "--locked",
        "--features",
        "bundled",
        "--bin",
        "i",
        "--profile",
        &args.profile,
        "--target",
        &host,
        "--target-dir",
    ]);
    cmd.arg(root.join("target"));

    if args.outline {
        cmd.args([
            "--",
            "-C",
            "llvm-args=-enable-machine-outliner=always",
            "-C",
            "llvm-args=-machine-outliner-reruns=2",
            "-C",
            "llvm-args=-enable-linkonceodr-outlining",
        ]);
    }

    cmd.current_dir(root);

    let status = cmd.status().context("Failed to run cargo build")?;

    if !status.success() {
        bail!("cargo build failed");
    }

    Ok(())
}

fn generate_config_example(root: &Path, package_dir: &Path) -> Result<()> {
    let source = fs::read_to_string(root.join("examples/api.openai.json"))?;

    // Parse and remove "plugins" field
    let mut config: serde_json::Value = serde_json::from_str(&source)?;
    if let Some(obj) = config.as_object_mut() {
        obj.remove("plugins");
    }

    let output = serde_json::to_string_pretty(&config)?;
    fs::write(package_dir.join("config.example.json"), output + "\n")?;

    Ok(())
}

fn generate_build_json(
    package_dir: &Path,
    version: &str,
    host: &str,
    profile: &str,
    outline: bool,
    binary_size: u64,
) -> Result<()> {
    let rustc_version = Command::new("rustc").arg("-vV").output().context("Failed to run rustc")?;

    let rustc_full = String::from_utf8_lossy(&rustc_version.stdout).lines().next().unwrap_or("unknown").to_string();

    let rustc_flags = if outline {
        serde_json::json!([
            "-C",
            "llvm-args=-enable-machine-outliner=always",
            "-C",
            "llvm-args=-machine-outliner-reruns=2",
            "-C",
            "llvm-args=-enable-linkonceodr-outlining"
        ])
    } else {
        serde_json::Value::Null
    };

    let build_info = serde_json::json!({
        "version": version,
        "target": host,
        "profile": profile,
        "layout": "bundled",
        "tls": "model-rustls-mozilla/web-platform-native",
        "rustc": rustc_full,
        "rustc_flags": rustc_flags,
        "binary_bytes": {
            "i": binary_size
        },
        "binary_total_bytes": binary_size,
        "known_issues": [
            "Scalar-based editor, not full Unicode graphemes",
            "Windows terminal execution not locally verified"
        ],
        "developer_id_signed": false,
        "notarized": false
    });

    let output = serde_json::to_string_pretty(&build_info)?;
    fs::write(package_dir.join("BUILD.json"), output + "\n")?;

    Ok(())
}

fn generate_sha256sums(package_dir: &Path) -> Result<()> {
    let mut files: Vec<PathBuf> = Vec::new();
    collect_files(package_dir, &mut files)?;
    files.sort();

    let mut sums = String::new();
    for file in files {
        let content = fs::read(&file)?;
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let hash = hasher.finalize();
        let hex = hex::encode(hash);
        let relative = file.strip_prefix(package_dir)?.to_string_lossy().replace('\\', "/");
        sums.push_str(&format!("{}  {}\n", hex, relative));
    }

    fs::write(package_dir.join("SHA256SUMS"), sums)?;
    Ok(())
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}

fn calculate_dir_size(dir: &Path) -> Result<u64> {
    let mut total = 0u64;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            total += calculate_dir_size(&path)?;
        } else {
            total += fs::metadata(&path)?.len();
        }
    }
    Ok(total)
}

fn create_tar_archive(
    temp_dir: &Path,
    dist_dir: &Path,
    package_name: &str,
    binary_size: u64,
    unpacked_size: u64,
) -> Result<()> {
    let archive_name = format!("{}.tar.gz", package_name);
    let archive_path = dist_dir.join(&archive_name);
    let temp_archive = temp_dir.join(&archive_name);

    println!("  Creating {}...", archive_name);

    let file = fs::File::create(&temp_archive)?;
    let package_path = temp_dir.join(package_name);

    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::new(9));
    let mut tar = tar::Builder::new(encoder);
    tar.follow_symlinks(false);
    tar.append_dir_all(package_name, &package_path)?;
    let encoder = tar.into_inner()?;
    encoder.finish()?;

    fs::rename(&temp_archive, &archive_path)?;

    // Generate checksum
    let archive_content = fs::read(&archive_path)?;
    let mut hasher = Sha256::new();
    hasher.update(&archive_content);
    let hash = hasher.finalize();
    let hex = hex::encode(hash);

    let sha_path = dist_dir.join(format!("{}.sha256", archive_name));
    fs::write(&sha_path, format!("{}  {}\n", hex, archive_name))?;

    let archive_size = fs::metadata(&archive_path)?.len();

    // Print summary
    let summary = serde_json::json!({
        "archive": archive_path.to_string_lossy(),
        "binary_bytes": { "i": binary_size },
        "binary_total_bytes": binary_size,
        "unpacked_file_bytes": unpacked_size,
        "archive_bytes": archive_size,
        "sha256": hex
    });

    println!("{}", serde_json::to_string_pretty(&summary)?);

    Ok(())
}

fn create_zip_archive(
    temp_dir: &Path,
    dist_dir: &Path,
    package_name: &str,
    binary_size: u64,
    unpacked_size: u64,
) -> Result<()> {
    let archive_name = format!("{}.zip", package_name);
    let archive_path = dist_dir.join(&archive_name);
    let temp_archive = temp_dir.join(&archive_name);

    println!("  Creating {}...", archive_name);

    let file = fs::File::create(&temp_archive)?;
    let mut zip_writer = zip::ZipWriter::new(file);
    let package_path = temp_dir.join(package_name);

    // Add all files to zip
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let exec_options = options.unix_permissions(0o755);
    let file_options = options.unix_permissions(0o644);

    add_dir_to_zip(&mut zip_writer, &package_path, Path::new(package_name), &exec_options, &file_options)?;

    zip_writer.finish()?;

    fs::rename(&temp_archive, &archive_path)?;

    // Generate checksum
    let archive_content = fs::read(&archive_path)?;
    let mut hasher = Sha256::new();
    hasher.update(&archive_content);
    let hash = hasher.finalize();
    let hex = hex::encode(hash);

    let sha_path = dist_dir.join(format!("{}.sha256", archive_name));
    fs::write(&sha_path, format!("{}  {}\n", hex, archive_name))?;

    let archive_size = fs::metadata(&archive_path)?.len();

    // Print summary
    let summary = serde_json::json!({
        "archive": archive_path.to_string_lossy(),
        "binary_bytes": { "i": binary_size },
        "binary_total_bytes": binary_size,
        "unpacked_file_bytes": unpacked_size,
        "archive_bytes": archive_size,
        "sha256": hex
    });

    println!("{}", serde_json::to_string_pretty(&summary)?);

    Ok(())
}

fn add_dir_to_zip<W: Write + io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    dir: &Path,
    prefix: &Path,
    exec_options: &zip::write::SimpleFileOptions,
    file_options: &zip::write::SimpleFileOptions,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = prefix.join(path.strip_prefix(dir)?);

        if path.is_dir() {
            zip.add_directory(name.to_string_lossy().replace('\\', "/"), *exec_options)?;
            add_dir_to_zip(zip, &path, &name, exec_options, file_options)?;
        } else {
            zip.start_file(name.to_string_lossy().replace('\\', "/"), *file_options)?;
            let content = fs::read(&path)?;
            zip.write_all(&content)?;
        }
    }
    Ok(())
}

// Simple hex encoding (to avoid adding hex crate dependency)
mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes.as_ref().iter().map(|b| format!("{:02x}", b)).collect()
    }
}
