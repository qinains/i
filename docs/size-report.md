# Native release size report — HTTPS regression corrected

> Historical native measurements: these predate the shared-core/browser WASM implementation. The current `src/` is 3165 physical Rust lines excluding `src/tests.rs`, including packaging/completions and inline tests; runtime/vendor/JS/assets are additional. The 1024-line cap and "current" size/test claims below describe that earlier snapshot only. Native distribution size has not been remeasured after this refactor. See [browser build and actual verification boundaries](wasm.md).

Measured on macOS arm64 (`aarch64-apple-darwin`), rustc 1.95.0. **The earlier 1.91 MiB executable / 0.72 MiB archive was broken for HTTPS and is not a valid full-function size result.** Do not use that build. Core production Rust is now **1024 physical lines**; the selected provider implementation is additional code, not part of a claimed 1024-line complete client.

## Current deployment / setup build

Adds Rust user-level self-installation, offline connection setup, optional host-side
OS credential retrieval, and portable built-in binding resolution. Existing config,
sessions and custom bindings are preserved; migration is explicit. Core is 1024 lines;
the deployment/credential implementation is additional runtime code, not hidden in
that cap. OS-store modes require that additional code.

| Artifact | Bytes |
|---|---:|
| Installed bundled executable, ordinary `min-size` | 2,338,144 |
| Packaged executable, `min-size --outline` | 2,307,136 |
| Unpacked outlined package payloads | 2,322,886 |
| XZ download | 919,292 |
| Gzip download | 1,214,314 |

XZ SHA256: `e6a16430f7c7c71864ebdd2da18e082ad82cfc5350a9a28bfcae4df1cf81567b`.
Gzip SHA256: `12d058d1b65dec583a6bbe9b1d3405399b1d19a261e087d100dba27413d8b817`.
188 bundled / 182 non-bundled Rust tests pass on the recorded native host. Five
installed deployment tests and twelve installed Files tests pass, as do model/web
TLS, console PTY and setup hidden-input/cancellation checks. Both archives pass
self-install, source-directory relocation and real bundled-tool fixture checks.
Formatting, Clippy, core cap and globset integrity/graph checks pass.
No live provider requests, personal configuration edits or personal credential-store
operations were performed. Actual Keychain read/write and Linux/Windows execution
remain unverified. Optional Linux OS storage additionally needs secret-tool/Secret
Service; the measured macOS build uses native system APIs. No remote downloader,
publisher signing/notarization or release publication is claimed.
The outlined executable increases by 50,416 bytes over the preceding GC build;
this is an artifact delta, not function-size or runtime-memory attribution.
Receipt: `dist/analysis/deployment-install-receipt.json`.

## Previous GC diagnostics / inspection build

Adds classified GC reports and read-only retired-file inspection. Invalid cached
IDs are retained for review, not silently refreshed or pruned. Core remains1022 lines.

| Artifact | Bytes |
|---|---:|
| Installed bundled executable, ordinary `min-size` | 2,287,824 |
| Packaged executable, `min-size --outline` | 2,256,720 |
| Unpacked outlined package payloads | 2,271,797 |
| XZ download | 899,252 |
| Gzip download | 1,187,500 |

XZ SHA256: `f1e49c5e84bb78a21170066b6360af125788ff362b4c7aac93bc190ad6023afc`.
Gzip SHA256: `9402c563ff8302f8d64931ea74c1bd4efe9d6677bd5922af797fae1b78038ade`.
181 bundled / 180 non-bundled tests pass; twelve Files integration tests also pass
against the installed binary. Installed model/web TLS, macOS PTY, fresh package,
Clippy, formatting, core cap and globset checks pass. No live provider calls.
The executable increases by 48 bytes versus the preceding build; this is an
artifact-size delta, not an estimate of function size or runtime memory.

## Previous multi-provider Files / lifecycle build

Includes OpenAI Responses / Anthropic beta Files, explicitly enabled refresh and
bounded collection of superseded owned uploads. The generic host adds a separate
cleanup capability. `--yolo` / `--dangerously-skip-permissions` replace `--yes`.

| Artifact | Bytes |
|---|---:|
| Installed bundled executable, ordinary `min-size` | 2,287,776 |
| Packaged executable, `min-size --outline` | 2,256,672 |
| Unpacked outlined package payloads | 2,271,437 |
| XZ download | 896,656 |
| Gzip download | 1,184,340 |

XZ SHA256: `e6f33426277935eeaa75e7a0d7f846102c34e013a25c1508be0a43f46c03fcde`.
Gzip SHA256: `e5ef028f1082d1b7f2cea65150fc172fd8f3aa708c5de856f246e7c9999e097a`.
178 bundled / 177 non-bundled tests pass, including both flag aliases and old-name
rejection. All eleven Files integration tests pass against the installed binary.
Installed strict model/web TLS and macOS PTY checks, Clippy, fmt, core cap,
globset integrity/graph, and freshly rebuilt package checks pass. No live provider
API calls, new HTTP/SDK dependencies, or personal configuration/session changes.
Outlined executable growth over the previous build: 16,688 bytes.

## Previous remote-management build

Remote Files list/get/delete/reconcile now share the core's bounded transport.
DELETE is restricted to explicit provider management and always requires confirmation.

| Artifact | Bytes |
|---|---:|
| Installed bundled executable, ordinary `min-size` | 2,271,120 |
| Packaged executable, `min-size --outline` | 2,239,984 |
| Unpacked outlined package payloads | 2,253,938 |
| XZ download | 888,288 |
| Gzip download | 1,174,401 |

XZ SHA256: `c24117c6dea7e285b52ef942926bff8c70d0f3b35138cdeff2535aae6a2b71b2`.
Gzip SHA256: `dcd339a1c8439a86907545b0b798cc342e64b329f37b78dd444d90078b93f98c`.
170 bundled / 169 non-bundled tests pass; all nine Files integration tests also pass
against the installed executable. Installed model/web TLS, macOS PTY, formatting,
Clippy, globset graph/integrity and fresh-package checks pass. No live provider calls.
The outlined executable grows by 16,656 bytes versus the preceding Files build;
only the compressed download remains below 1 MiB. No new SDK/HTTP dependencies.

## Previous Files-capable provider build

DeepSeek Files uploads are explicitly opted in; HTTP, multipart-body bounds,
credential injection and no-retry auxiliary transport remain in the core.
Provider-specific encoding and recoverable mappings are counted separately.

| Artifact | Bytes |
|---|---:|
| Installed bundled executable, ordinary `min-size` | 2,254,480 |
| Packaged executable, `min-size --outline` | 2,223,328 |
| Unpacked outlined package payloads | 2,236,780 |
| XZ download | 882,240 |
| Gzip download | 1,165,501 |

XZ SHA256: `9360be7a221b9fb2ac9828dd6857ade274878e895db1dbb2fcc6d9e85ce67f11`.
Gzip SHA256: `d30070057777bf057d0ef5864ea65de3b32340c9727756dd345c0c0c48a8628e`.
164 bundled / 163 non-bundled tests pass; all five Files integration tests also pass
against the installed executable, including multipart, resume, credential rotation,
no retries, explicit opt-in, operation limits and killing the host after upload bytes.
Installed TLS/PTTY, formatting, Clippy and fresh-package checks pass.
The outlined executable is 16,672 bytes larger than the preceding provider build.
Only the compressed download is below 1 MiB; no throughput or real API/cache claim is made.
sha2/base64 are reused from the existing dependency graph; no new HTTP/SDK stack was added.

## Previous provider-worker build

Vendor wire formats now live in replaceable provider workers; HTTP, retries,
credential injection and worker transport remain in the core. See [providers.md](providers.md)
for the changed core-only deployment requirement and explicit source accounting.

| Artifact | Bytes |
|---|---:|
| Installed bundled executable, ordinary `min-size` | 2,237,824 |
| Packaged executable, `min-size --outline` | 2,206,656 |
| Unpacked outlined package payloads | 2,219,584 |
| XZ download | 874,400 |
| Gzip download | 1,154,827 |

Current XZ SHA256: `9e2f1fc79ffc8b6be47a1601755c032fad8316cfbeac4f404d53594fbeaf902e`.
Current gzip SHA256: `d1a351c4fe4d1e1471546b6784b86224da5a280f49f4c368ec957db8024676b3`.
The min-size archives at the paths below have been replaced; old extracted directories
and the previous standard-release package have not been updated.
153 bundled / 152 non-bundled tests, Clippy, formatting, actual-package checks,
strict model/web TLS tests and installed-binary macOS PTY checks pass.
No paid provider calls or new dependency-size attribution were performed.
This is architectural decoupling, not evidence of improved throughput or a smaller full client.

## Previous local build: Markdown and replaceable approval modes

The newer working tree retains the user's optional Markdown changes and adds the
replaceable `approval` policy plus `/mode ask|auto` (`/yes` aliases). On the same
macOS arm64 toolchain, measured artifacts are:

| Artifact | Bytes |
|---|---:|
| Installed bundled executable, ordinary `min-size` | 2,237,728 |
| Packaged bundled executable, `min-size --outline` | 2,206,528 |
| Unpacked outlined package payloads | 2,219,009 |
| XZ download | 869,384 |
| Gzip download | 1,149,433 |

Archive paths (now replaced by the provider-worker build above): `dist/i-0.1.0-aarch64-apple-darwin-min-size.tar.xz` and `.tar.gz`.
XZ SHA256: `05879598af95595c6a2d34ad7ff6f9df2c05a94dbe7386f9cafd530bfe3f6a15`.
Gzip SHA256: `23bb29707b33c83106e3fab4539fbd3c472060c5a2c16816cb0716751d189d7d`.
These archives replace the previous files at those paths; old extracted directories
remain untouched. Standard `release` was not rebuilt for this update.

142 bundled / 141 non-bundled workspace tests pass, including the 1024-line guard,
custom approval binding execution and failure without fallback. Formatting, Clippy,
strict model/web TLS rejection, package checks and macOS PTY mode-switch tests pass.
The installed executable was separately checked with strict TLS and PTY tests.
No paid model request was made. Optional code and package metadata grew; dependency
symbol attribution below/in linked reports was not rerun for these feature additions.

## What went wrong

Changing ureq 3.4.2 from `native-tls` to `native-tls-no-default` removed more than bundled certificate data. That version guards its actual native TLS module/connector and provider availability check with `native-tls`. HTTPS therefore panicked before sending ClientHello. Both production manifests now restore the supported `native-tls` feature; system certificate/hostname validation remains enabled.

The previous self-signed certificate test accepted any nonzero exit plus a server-side SSL error. A pre-handshake panic caused an EOF SSL error and incorrectly passed. This false positive has been fixed: the test requires ClientHello, a certificate/trust diagnostic, a normal error exit or worker error response, no HTTP application data, no panic and no retry. It rejects the old broken executable. The generated certificate now includes the correct IP/hostname SAN, so hostname mismatch cannot substitute for trust rejection.

## Previous HTTPS-fixed measurement (before Markdown/approval changes)

| Artifact | Standard release | min-size + outlining |
|---|---:|---:|
| `bin/i`, core + optional runtime | 5,402,736 bytes / 5.15 MiB | 2,189,872 bytes / 2.09 MiB |
| All unpacked file payloads | 5,414,633 bytes / 5.16 MiB | 2,201,942 bytes / 2.10 MiB |
| `.tar.gz` package | 2,145,270 bytes / 2.05 MiB | 1,141,635 bytes / 1.09 MiB |
| **`.tar.xz` package** | **1,452,028 bytes / 1.38 MiB** | **863,936 bytes / 0.82 MiB** |

Previous preferred archive path (now rebuilt as described above): `dist/i-0.1.0-aarch64-apple-darwin-min-size.tar.xz`.

- XZ SHA256: `6990209e3f961cdb0d8ab3f5b76dae00dba2ea8350b9d11ae996e2892a53891b`
- Gzip SHA256: `d83228e27e88cae1630cfe610c1d3771010e492c19085774909543afca72a950`

A verified fresh extraction is available locally at `dist/i-0.1.0-aarch64-apple-darwin-min-size-https-fixed/`. Older extracted directories were not overwritten: their binaries do not update when archives are rebuilt. Exit the old console and start the new executable. Pass your actual project directory to `start.sh`; a release `bin/` directory should not accidentally become your coding workspace. Existing settings and sessions are not deleted or migrated.

Both formats contain identical files. Ordinary min-size **without** the optional outlining flags is **2,221,088 bytes / 2.12 MiB**. Native-target packages use `target/aarch64-apple-darwin/PROFILE/i`; `target/min-size/i` is the ordinary build. `BUILD.json` records packaging flags (unknown/null with `--skip-build`). Only the XZ download is below 1 MiB; the uncompressed complete program is not.

## Optimizations retained

- One executable shares the core and optional runtime. Worker processes still have bounded NDJSON transport and isolated environments; merging source crates is not used to hide core code.
- Synchronous ureq/platform TLS replaces reqwest/hyper/tokio/rustls/ring. **Native TLS is now actually enabled.** Ureq's bundled Mozilla certificate data remains present even though clients explicitly select platform trust. Its removal is not claimed.
- Full Unicode syntax and glob semantics remain supported. The pinned globset copy changes only regex backend features; upstream Rust sources are hash-identical. Literal prefiltering is retained. Extra DFA/backtracking engines were removed, with potential workload-dependent speed tradeoffs.
- Base64 versions are unified. URL encoding reuses url and the existing ryu formatter; serde_urlencoded is a test-only oracle. Full YAML support and Anyhow diagnostics remain enabled.
- Standard XZ/LZMA2 compression, canonical archive headers and optional AArch64 outlining reduce distribution size. No self-extracting loader, downloaded helper or new runtime library is involved.

The original valid 2.23 MiB bundled build and earlier corrected 2.09 MiB build are meaningful full-function comparison points. Intermediate no-default-TLS builds and their apparent root-certificate savings are invalid as working HTTPS distributions. See the [corrected dependency analysis](dependency-size.md).

## Reproduce and validate

```sh
python3 scripts/package.py --profile release
python3 scripts/package.py --profile min-size --outline  # AArch64 only
python3 scripts/check_package.py 'dist/*.tar.gz' 'dist/*.tar.xz'
python3 scripts/check_tls.py target/aarch64-apple-darwin/min-size/i --web
python3 scripts/check_console.py target/aarch64-apple-darwin/min-size/i
python3 scripts/check_globset.py --graph
# Other architectures: omit --outline.
```

The outlined build uses opt-level=z, fat LTO, one codegen unit, stripping and panic=abort, plus `-enable-machine-outliner=always`, `-machine-outliner-reruns=2`, and `-enable-linkonceodr-outlining`. LLVM options are toolchain-specific and tested locally on rustc 1.95.0/macOS arm64. Unsupported flags fail rather than silently falling back. Panic abort cannot run cleanup guards; ordinary errors and handled signals retain cleanup.

For that earlier measurement, **130 bundled workspace Rust tests / 129 without bundled** passed, including a new portable HTTPS ClientHello/no-panic regression. The unchanged globset patch passed 295 upstream tests and 5 doc tests; source integrity and resolved-feature guards pass. Formatting and Clippy with warnings denied pass. Both packaged profiles/formats pass checksums, actual console tasks, tools, Unicode search, budget and cache checks. On non-Windows hosts, package checking now invokes the strict TLS test against **each extracted executable**, covering both model HTTP and web worker paths. Packaged PTY checks pass. A public HTTPS GET to example.com also succeeded; no paid model API or user credential was used for that check.

macOS dynamically uses system Security/Foundation/CoreFoundation; Windows uses Schannel; Linux requires OpenSSL and system CA certificates. System libraries/trust stores, personal state and build caches are not included in archive figures. Python/OpenSSL executables are test/build utilities, not runtime agent dependencies. Only macOS was locally executed. Signing/notarization, universal binaries and a complete dependency-license audit remain outside these claims.

## Source accounting

| Mandatory production Rust | Lines |
|---|---:|
| `src/main.rs` | 873 |
| `src/net.rs` | 73 |
| `src/worker.rs` | 78 |
| **Total** | **1024 / 1024** |

Blank lines/comments count; `build.rs` would count if present. Only the dedicated core test module is excluded. At the earlier HTTPS-fixed measurement, first-party optional runtime code was 2891 lines (3915 combined); those optional-code figures predate Markdown and the approval plugin and are not current totals. Separately, vendored globset contains 3405 existing upstream Rust lines including tests/benches, previously fetched from the registry. HTTP/retry/worker transport was not relocated. Vendor encoding has moved to a replaceable provider: its six source files add 1076 lines, giving 2100 for core plus those files, before Responses/runtime support. A provider is required to call models. Current runtime source files total 5365 lines excluding dedicated test files but including inline tests; combined on that basis, 6389 lines. Deployment and OS credential-storage implementations are included in that runtime count. Credentials are retrieved in the host process, not through provider RPC; this extra code is required when using OS-store mode.
