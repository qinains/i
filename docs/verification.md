# Cross-platform verification

Verified source commit: `e409fc665d37724ea193bc37b8d73d78ad36e69c`.

GitHub Actions: [six-platform acceptance run](https://github.com/qinains/i/actions/runs/44214997537).
The verification branch is `ci/cordis-acceptance-20261118T003215Z`.
Documentation-only descendants do not change the tested executable sources.
The overall run is **not green**: all native jobs and Chromium passed, but
both WebKit attempts failed one test each.

## Native matrix

These are native-architecture GitHub-hosted runners, not local ARM-to-x64
emulation. They are not a bare-metal performance benchmark.

| Runner | Target | Default tests | Bundled + packaging tests |
| --- | --- | ---: | ---: |
| ubuntu-latest | x86_64-unknown-linux-gnu | 373 | 386 |
| ubuntu-24.04-arm | aarch64-unknown-linux-gnu | 373 | 386 |
| macos-latest | aarch64-apple-darwin | 373 | 386 |
| macos-15-intel | x86_64-apple-darwin | 373 | 386 |
| windows-latest | x86_64-pc-windows-msvc | 360 | 372 |
| windows-11-arm | aarch64-pc-windows-msvc | 360 | 372 |

All six native jobs passed. Each test matrix also retains one explicitly
ignored historical 1,024-line guard. Platform-specific test counts differ.
The jobs additionally check strict Clippy, formatting, minimal features,
documentation tests, upstream globset tests, and Python script regressions.
Unix runners also pass real PTY editing/setup restoration and offline TLS
rejection checks; those dedicated checks are not run by Windows CI.

## Packages

All six targets build optimized `min-size` packages. Each gzip/xz package, plus
each Windows ZIP, passes offline integration checks on its corresponding runner:

- SHA256 integrity, build metadata, and a single bundled executable;
- self-installation followed by source-directory relocation;
- portable worker bindings, all 17 advertised tools, and a synthetic model task;
- Unicode literal/regex search, bundled memory workers, and usage accounting.

All 14 archives were downloaded separately for an independent SHA256,
target/profile/layout, binary-size, and raw ZIP-name audit. All passed.
In particular, Windows ZIP member names contain forward slashes, not Windows
path separators. An earlier CI-green snapshot failed this independent audit;
its ZIP writer and validator were corrected before this verification.

Windows relocation verification allows only a bounded three-second wait for
Win32 access/sharing errors on the exact directory rename. Persistent errors
and unrelated failures still fail the check; installation is not replayed.
The specific owner of the transient access denial seen in an earlier run
was not identified.

## Browser and Cordis

CI rebuilds WASM and Cordis assets from committed source; locally ignored
generated bindings are not included in the verification snapshot.
The Chromium suite passed 144 tests. Node unit tests passed 150 tests and
Cordis build tests passed five tests. Python script tests passed 17 tests.

The first WebKit attempt passed 143 of 144 tests. The two-real-plugin test
reported the JSON plugin unavailable during task snapshot acquisition, then
failed waiting for approval. Its unavailable-plugin cause was not retained by
the existing snapshot API. An unchanged-code full WebKit rerun also passed
143/144, but the two-plugin test passed and a different file-preview test
failed during `browserContext.newPage` with the context/browser already closed.
The second failure occurred before that test's application assertions.
These different failures do not establish a shared cause or a runtime fix.
Local controls passed the same two-plugin test ten times, then both the
two-plugin and offline-plugin-PWA tests five times each. These controls do not
prove that the intermittent failure is fixed. No speculative runtime fix,
automatic test retry, or removed assertion is used to hide it.

## Remaining boundaries

- The intermittent WebKit plugin-unavailable failure still needs a captured
  underlying inspection/storage error before claiming a root-cause fix.
  The separate WebKit context/page startup failure also remains unexplained.
- Windows ARM64 local TTY checks cover editing, approvals, cancellation and
  terminal-mode restoration, not full resize/soak parity or x64 TTY behavior.
- Prior bounded DeepSeek `deepseek-flash` synthetic live checks cover native
  tools/approvals/session recovery and real browser WASM streaming/reload/cancel.
  This CI run uses no real provider credentials or paid model requests.
- Other provider accounts, Developer ID distribution signing, Apple
  notarization, and performance improvements are not verified.
- No merge, tag, GitHub Release publication or Pages deployment was performed.
  Build/integration evidence is not formal signed-distribution acceptance.
