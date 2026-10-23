#!/usr/bin/env python3
"""Development packaging utility. Distributed programs need neither Python nor Rust."""
import argparse
import hashlib
import gzip
import json
import lzma
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]
README = """i / lninl — native CLI release build

Contents: bin/i (core + optional runtime), config.example.json, start.sh, logo.svg,
BUILD.json and SHA256SUMS. No Python/Node/Rust installation is needed to run
these binaries. The platform's system libraries are still required.

Quick start (macOS/Linux):
  1. Run bin/i install (default ~/.local/bin; Windows: LocalAppData/Programs/i/bin).
     Existing different executables require confirmation or --replace; backups kept.
     --add-to-path explicitly authorizes a user PATH/profile update. No sudo needed.
  2. Run the installed i setup: provider, URL (official default), model and key.
  3. Run i start, bare i in a terminal, or ./start.sh [workspace-directory].
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

Networking uses synchronous HTTP and platform TLS, with certificate/hostname
verification enabled. macOS/Windows use system TLS; Linux requires OpenSSL and
system CA certificates. No external curl/Python process handles networking.
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
Full regex Unicode syntax remains enabled. Globset uses unmodified upstream
Rust sources with narrower backend features: literal prefiltering is retained,
extra DFA/backtracking engines omitted. Search performance may differ.
The globset MIT notice is included; this is not a complete dependency-license audit. TLS uses system trust. The pinned ureq version also bundles Mozilla root data;
its native-tls feature must remain enabled for the actual HTTPS connector.
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
This local package is not a dependency-license audit/publication artifact.

Sizes exclude personal configuration, sessions, metrics, caches, and build
artifacts. SHA256SUMS covers every other packaged file, including BUILD.json.
"""
LAUNCHER = """#!/bin/sh
# Launch packaged Rust programs; no model or tool logic lives in this wrapper.
set -eu
if [ "$#" -gt 1 ]; then
    printf 'Usage: ./start.sh [workspace]\\n' >&2
    exit 2
fi
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
exec "$here/bin/i" start --dir="${1:-$PWD}"
"""


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profile', choices=['release', 'min-size'], default='min-size')
    parser.add_argument('--skip-build', action='store_true', help='package existing native binaries')
    parser.add_argument('--compression', choices=['both', 'xz', 'gzip'], default='both')
    parser.add_argument('--outline', action='store_true', help='extra AArch64 min-size LLVM outlining; requires a build')
    args = parser.parse_args()
    rustc = subprocess.check_output(['rustc', '-vV'], text=True)
    host = next(line.split(': ', 1)[1] for line in rustc.splitlines() if line.startswith('host: '))
    version = re.search(r'^version = "([^"]+)"', (ROOT / 'Cargo.toml').read_text(), re.M).group(1)
    if args.outline and (args.skip_build or args.profile != 'min-size' or not host.startswith('aarch64-')):
        parser.error('--outline requires an AArch64 min-size build, without --skip-build')
    rustc_flags = ['-C', 'llvm-args=-enable-machine-outliner=always',
                   '-C', 'llvm-args=-machine-outliner-reruns=2',
                   '-C', 'llvm-args=-enable-linkonceodr-outlining'] if args.outline else []
    if not args.skip_build:
        command = ['cargo', 'rustc' if args.outline else 'build', '--locked', '--features', 'bundled',
                   '--bin', 'i', '--profile', args.profile, '--target', host, '--target-dir', str(ROOT / 'target')]
        subprocess.run(command + (['--', *rustc_flags] if rustc_flags else []), cwd=ROOT, check=True)
    # Explicit-target builds and the existing native no-target builds have different directories.
    binaries = ROOT / 'target' / host / args.profile
    if args.skip_build:
        binaries = ROOT / 'target' / args.profile
    suffix = '.exe' if 'windows' in host else ''
    name = f'i-{version}-{host}-{args.profile}'
    dist = ROOT / 'dist'
    dist.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.package-', dir=dist) as temporary:
        package = Path(temporary) / name
        (package / 'bin').mkdir(parents=True)
        sizes = {}
        subprocess.run([str(binaries / ('i' + suffix)), '--runtime', 'logo', '--plain'],
                       stdout=subprocess.DEVNULL, check=True)
        if 'windows' not in host:
            subprocess.run([sys.executable, str(ROOT / 'scripts/check_tls.py'),
                            str(binaries / ('i' + suffix)), '--web'], cwd=ROOT, check=True, timeout=45)
        for command in ('i',):
            source = binaries / (command + suffix)
            destination = package / 'bin' / source.name
            shutil.copy2(source, destination)
            sizes[command] = destination.stat().st_size
        settings = json.loads((ROOT / 'examples/api.openai.json').read_text())
        settings.pop('plugins', None)  # The launcher supplies a newly generated packaged-worker manifest.
        (package / 'config.example.json').write_text(json.dumps(settings, indent=2) + '\n')
        (package / 'README.txt').write_text(README)
        (package / 'start.sh').write_text(LAUNCHER)
        (package / 'start.sh').chmod(0o755)
        shutil.copy2(ROOT / 'assets/lninl.svg', package / 'logo.svg')
        shutil.copy2(ROOT / 'vendor/globset/LICENSE-MIT', package / 'LICENSE-globset.txt')
        metadata = dict(version=version, target=host, profile=args.profile, layout='bundled',
                        tls='platform-native', rustc=rustc.splitlines()[0],
                        rustc_flags=rustc_flags if not args.skip_build else None,
                        binary_bytes=sizes, binary_total_bytes=sum(sizes.values()),
                        known_issues=['Scalar-based editor, not full Unicode graphemes', 'Windows terminal execution not locally verified'],
                        developer_id_signed=False, notarized=False)
        (package / 'BUILD.json').write_text(json.dumps(metadata, indent=2) + '\n')
        files = sorted(path for path in package.rglob('*') if path.is_file())
        (package / 'SHA256SUMS').write_text(''.join(f'{digest(path)}  {path.relative_to(package).as_posix()}\n' for path in files))
        unpacked = sum(path.stat().st_size for path in package.rglob('*') if path.is_file())
        # Canonical headers avoid leaking host usernames/paths and make repeat packaging stable.
        def canonical(info):
            info.uid = info.gid = info.mtime = 0
            info.uname = info.gname = ''
            info.pax_headers = {}
            info.mode = 0o755 if info.isdir() or info.name.endswith(('/bin/i', '/bin/i.exe', '/start.sh')) else 0o644
            return info
        # Standard LZMA2 only: no custom loader/filter or runtime dependency.
        dictionary = max(1 << 22, min(1 << 26, 1 << (unpacked + 65535).bit_length()))
        xz_filter = dict(id=lzma.FILTER_LZMA2, preset=9, dict_size=dictionary)
        if host.startswith('aarch64-'):
            xz_filter.update(lc=2, lp=2, pb=2)  # Four-byte instruction alignment.
        for extension in ('xz', 'gz'):
            if args.compression != 'both' and extension != {'xz': 'xz', 'gzip': 'gz'}[args.compression]:
                continue
            archive = dist / f'{name}.tar.{extension}'
            temporary_archive = Path(temporary) / archive.name
            with temporary_archive.open('wb') as raw:
                compressor = (lzma.LZMAFile(raw, 'w', filters=[xz_filter]) if extension == 'xz' else
                              gzip.GzipFile(filename='', mode='wb', fileobj=raw, compresslevel=9, mtime=0))
                with compressor, tarfile.open(fileobj=compressor, mode='w|') as output:
                    output.add(package, arcname=name, filter=canonical)
            temporary_archive.replace(archive)
            (dist / f'{archive.name}.sha256').write_text(f'{digest(archive)}  {archive.name}\n')
            print(json.dumps(dict(archive=str(archive), binary_bytes=sizes, binary_total_bytes=sum(sizes.values()),
                                  unpacked_file_bytes=unpacked, archive_bytes=archive.stat().st_size,
                                  sha256=digest(archive)), indent=2))


if __name__ == '__main__':
    main()
