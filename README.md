# i · lninl

![lninl — the central i](assets/lninl.svg)

**Small core. Your tools. Your model.**

A plugin-first Rust coding agent with a shared Native/browser WASM core. Native tools and plugin tools share one execution pipeline; plugins run as persistent, bounded JSON-line workers. The expanded cross-platform implementation is no longer subject to the historical 1,024-line target. The central **i** in **lninl** is generated in Rust, not embedded in model prompts.

[简体中文](README.zh-CN.md) · [Website & docs](https://i.lninl.com/docs/) · [Write a plugin](docs/plugin-development.md) · [Contributing / PR](CONTRIBUTING.md) · [MIT](LICENSE)

## What is implemented

- [Browser WASM PWA](docs/wasm.md): isolated workspaces, independent conversation tabs, reload recovery of history/drafts, streaming Chat Completions, persistent OPFS files, approvals, ZIP downloads and offline startup. Direct mode is default; an [own same-origin relay](server/README.md) is optional for APIs without CORS. Shell/process plugins are disabled. Build with `./scripts/build_web.sh`, then publish static assets over HTTPS (localhost for development).
  Scratch workspaces and a separate `/tmp` mount live only in page memory. Isolated HTML previews disable scripts by default; trusted JavaScript can be explicitly enabled. Install Rust/WASM tool plugins as `.hplugin.zip` packages. This is not a backend server; see the [browser guide](docs/wasm.md) for preview capabilities and limits.

- Replaceable provider workers for OpenAI Chat/Responses, DeepSeek and Anthropic Messages; no vendor wire-format branches in the core. See [provider contracts and limits](docs/providers.md).
- Opt-in [DeepSeek Files uploads and scoped file_id reuse](docs/provider-files.md), with host-held credentials and no automatic upload retries; Base64 remains the default. [Remote Files management](docs/provider-files-management.md) supports list/get/delete and explicit reconciliation without generating a model response; ordinary DELETE always asks. [OpenAI Responses/Anthropic Files and opt-in lifecycle maintenance](docs/files-lifecycle.md) are also supported; automated cleanup requires a separate host capability.
- Read, write, exact edit, shell, and image tools; approval gates, validated history, atomic sessions, and bounded I/O.
- Built-in recursive `glob`/`search`, batch range reads and same-file batch edits reduce model round trips and irrelevant context; see [file tools](docs/file-tools.md).
- Optional Rust modules for context budgets, compaction, prompts, memory, web, Skills, cache accounting, and interactive steering.
- Persistent subprocess workers shared by identical argv + canonical working directory. Failed operations are **never automatically replayed**.
- Append-only reference snapshots/deltas, stable public tool schemas, and accounting from **actual provider usage**, not guessed cache hits.
- Desktop configuration under `~/.i/config`, with platform-supplied paths for persistent data, disposable cache, and temporary runtime files.

There is no core provider/model capacity table. Official runtime plugins and the console are Rust; Python is only used by development fixtures and optional examples.

## Install

### From source (recommended)

Requires Rust 1.91+. Build the audited repository rather than assuming a
same-named crates.io package or Homebrew tap is this project:

```sh
git clone https://github.com/qinains/i.git
cd i
cargo install --path . --locked --features bundled --bin i
i setup
```

Homebrew and registry installation are not advertised until those releases have
actually been published and verified. Browser WASM plugins use a separate SDK;
see the [plugin tutorial](docs/plugin-development.md).

### Manual Download

When a release is available, download only the matching published artifact from
[GitHub Releases](https://github.com/qinains/i/releases). The table describes
packaging targets, not a claim that every target is already published:

| Platform | Architecture | File |
|----------|--------------|------|
| macOS | ARM64 (Apple Silicon) | `.tar.gz` |
| macOS | x86_64 (Intel) | `.tar.gz` |
| Linux | x86_64 | `.tar.gz` |
| Linux | ARM64 | `.tar.gz` |
| Windows | x86_64 | `.zip` |
| Windows | ARM64 | `.zip` |

Extract and run:

```bash
# Unix
./bin/i install    # Install to ~/.local/bin
i setup           # Configure provider, model, API key
i start           # Start with current directory

# Windows
bin\i.exe install  # Install to %LOCALAPPDATA%\Programs\i
i setup
i start
```

New bundled configs resolve built-in workers from the currently running executable, including after relocation:

```text
~/.i/config/
  config.json       # connection settings and portable builtin manifest reference
  plugins.json      # portable profile selection and disabled module names
  builtin.json      # builtin_profile descriptor, no executable paths
  bindings.json     # optional explicit custom worker replacements
  cache.json        # observe-only cache accounting by default
```

Normally use the wizard. An advanced environment-variable configuration can look like this:

```json
{
  "protocol": "openai",
  "base_url": "https://api.deepseek.com",
  "model": "deepseek-flash",
  "api_key_env": "DEEPSEEK_API_KEY",
  "max_tokens": 8192,
  "use_max_tokens": true,
  "plugins": "builtin.json",
  "budget": {"enabled": false},
  "extra_body": {"thinking": {"type": "disabled"}}
}
```

For this example, set the named environment variable privately. The wizard also implements OS credential storage with hidden input and opaque references, not plaintext JSON keys. Actual OS-store read/write has not been exercised against personal stores in offline tests. **Never put keys in prompts, memory, Git, or CLI arguments.** Then run:

```sh
i --runtime console -C /path/to/project --session-name main
```

The desktop console supplies an explicit session path to the core and stores named console sessions outside the project. The default `standard` profile includes the optional capabilities except the Responses adapter; `full` also enables Responses. Use the appropriate wire protocol for your endpoint.

`config.json` is discovered automatically; explicit `--config` wins. CLI model, endpoint, protocol, output budget, plugin manifest and credential-variable flags override configuration. Model and endpoint can also come from `API_MODEL` and `API_BASE_URL`.

The CLI can run without the console; a provider worker is still required (included in bundled builds):

```sh
i --config /path/to/model.json -C /path/to/project \
  --session /path/to/session.json 'Inspect the project and propose a focused change'
```

Repository manifests under `plugins/` are development examples targeting `target/debug/i-plugins`. New bundled installs do not use them. Existing legacy `desktop.json` can be explicitly migrated with `i setup --migrate-builtins --confirm`; detected custom changes are refused and originals kept. Advanced legacy/static manifests can still be generated:

```sh
i --runtime manifest standard > /tmp/desktop.json
# After checking it, replace ~/.i/config/desktop.json with this generated file.
```

Only legacy/static manifests need regeneration after relocation or selection changes. New `builtin.json` resolves profile/disabled/custom bindings at startup. Explicit custom worker paths remain the user's responsibility; neither installation nor setup silently rewrites them.

## Interactive input

The console now edits Chinese input at UTF-8 character boundaries without adding third-party crates. Backspace/Delete, Left/Right, Home/End, in-memory history and bracketed paste are supported. Background output preserves the draft; `[task finished]` marks completion. Pasting does not submit embedded newlines on supporting terminals. The single-row viewport scrolls horizontally.

This is a scalar editor, not a full Unicode grapheme engine: unsupported glyphs, including emoji/combining marks, display as ASCII escapes in the editable row while submitted text is retained. Non-TTY/piped operation remains line-based. Normal exit/error/Ctrl-C restores terminal modes; SIGKILL or panic-abort cannot run cleanup. See [console behavior and limitations](docs/console.md).

## Storage and future mobile hosts

```text
~/.i/
  config/                         # configuration, plugin settings, optional prompt files
  data/
    workspaces.json               # canonical-project digest -> UUID, locked and atomic
    workspaces/<id>/
      sessions/                   # desktop console sessions
      memory.json                 # optional memory
      metrics/cache-stats.json    # durable usage statistics, not disposable cache
  cache/                          # regenerable local cache; no mandatory artifact yet
  runtime/                        # temporary console queues/IPC
```

Override locations independently with absolute `I_CONFIG_DIR`, `I_DATA_DIR`, `I_CACHE_DIR`, and `I_RUNTIME_DIR`. Worker requests can instead carry an explicit `paths` object and `workspace_id`; see the contract documentation. Persistent locations cannot be nested in disposable locations, including currently resolvable symlink aliases. These checks are not a filesystem sandbox.

```sh
i --runtime paths -C /path/to/project
```

Desktop IDs are registered against canonical project paths. Moving a project is not automatic state migration. An explicit mobile workspace ID is a host-owned binding, **not authorization**. Session files still validate their workspace binding.

The low-level core accepts arbitrary explicit `--session FILE` paths. Its standalone `--session-name NAME` / `--sessions list|show|delete` convenience interface remains project-local at `.i/sessions`; it is **not** the desktop console's global session namespace. Use the console or explicit paths for centralized storage. Skill source files may also be project-local.

The mobile boundary is a storage/request contract, **not a shipped mobile backend or SDK**. A future Android/iOS host must supply app-private directories, authorized workspace access, native credential storage, and an in-process/platform/remote execution backend. Arbitrary desktop subprocess plugins cannot be assumed on mobile.

## Tools, workers and approvals

Native and external tools use the same validation → approval → execution → result/image → history pipeline. `before_tool` and `after_tool` hooks surround execution. Complete call batches are validated before any execution, and checkpoints preserve call/result pairing.

- `read` and `view_image` do not require approval; writes, edits, shell, and **every external tool** do.
- `--yolo` (alias `--dangerously-skip-permissions`) selects auto approval; `--yes` was removed. Use only in trusted environments. Explicit ask/deny policies still apply; these flags do not grant Files cleanup permissions.
- File paths reject workspace escape. Reads reject non-regular files; images check type signatures, size, and paths.
- `--image FILE` and tool results support PNG/JPEG/GIF/WebP; at most 8 images per task/batch and 8 MiB per image. Aggregate transport limits can be reached earlier.
- Shell uses process-group timeouts (`--timeout`, default 60 seconds); Windows uses `cmd.exe /D /C` and Job Objects through `command-group`.
- Workers have a 15-second call deadline, including blocked writes, and 48 MiB request/response frame caps. Failure evicts and kills the worker; a **later call** can start a fresh one. No transparent retry of side effects.
- Workers are scoped to the running core process, not a daemon shared across separate CLI invocations.

Worker startup reuse is tested. **No measured speedup is claimed**: JSON copying, serialization, IPC, and network latency remain. Lifecycle hooks are trusted automatic code; external tools require approval, but neither is a sandbox. Shell inherits the normal environment except `I_APPROVAL_TOKEN`; workers use a small environment allowlist and do not inherit model-key variables by default.

## Interactive console and branding

```sh
i --runtime console --bin /path/to/i -C /path/to/project --session-name main
```

While a request or tool is running:

- `/steer TEXT`: apply at the next safe boundary.
- `/queue TEXT`: submit after the current task ends.
- `/approve`, `/deny`: answer a pending approval.
- `/mode ask|auto`: switch approval policy mode; `/mode` shows status. Replaceable policies can still ask or deny; see [approval plugins](docs/approval.md).
- `/exit`: finish the current task, then exit; Ctrl-C aborts the active task and cleans up the console child.

Applied steering is persisted and acknowledged only after a successful save. Pending inbox entries and queued follow-ups are temporary and are not crash-durable. Approval frames on stderr use a per-child token.

The logo is optional presentation code, outside the core limit:

```sh
i --runtime logo
i --runtime logo --plain --ascii
i --runtime logo --svg > lninl.svg
i --runtime console --no-logo -C /path/to/project
```

`NO_COLOR` disables ANSI color. The console prints the logo to its UI, never into prompts, tool schemas, or history. The SVG and terminal renderers share the same glyph data.

## Budgets, compaction and references

Enable the budget module and explicitly configure model metadata:

```json
{
  "budget": {
    "context_window": 1048576,
    "compact_ratio": 0.8,
    "safety_ratio": 0.02,
    "tool_output_ratio": 0.02,
    "image_tokens": 16384,
    "images": "keep"
  }
}
```

These are operator-provided example values, not a universal model specification. The default trigger is `min(window × 0.8, window − output reservation − safety)`. Tool-output allocation depends on remaining capacity and pending calls, rather than a fixed token cutoff. The separate `io_limit_bytes` safety cap defaults to 1 MiB.

Budgeting prioritizes returned input + output usage when its anchor matches the model and history prefix, then conservatively estimates new content and reserves a tool-schema allowance. First requests, changed prefixes/models, and incomplete or malformed usage use a lightweight UTF-8-byte estimate; images require an explicit planning value. Usage arrives after a request and cannot predict unseen input. There are no bundled BPE tokenizers/vocabularies; explicitly requesting a BPE encoding fails rather than silently switching algorithms. These estimates are not exact provider counts. Compaction compares consistent content estimates, and schemas contain no executable configuration.

Compaction is a separate, replaceable **`compact` worker**, not part of the budget implementation. `standard`/`full` include it; `compact` and `responses-compact` profiles also include the required budget module. The `budget`/`responses-budget` profiles now count/limit context without automatic compaction. Default retention, image handling and prompt files can be selected in `config/compact.json`.

Compaction preserves complete tool batches and recent history, saves a `.precompact.json` backup, and requires the proposed history to shrink by comparable token estimates and fit the budget. Summary requests are budget-checked **before HTTP**. Oversized summaries fail rather than silently overflowing; automatic chunked summarization is not implemented. Images may be kept, described, or omitted without changing original image files.

Memory and Skills append untrusted snapshots and subsequent deltas instead of rewriting system/history prefixes. Unchanged sources add nothing. Missing baselines after compaction are restored before the final budget check and commit. Compression remains lossy and can reset provider cache prefixes.

### Replace a built-in worker

Put host-specific overrides in `~/.i/config/bindings.json` (or `$I_CONFIG_DIR/bindings.json`):

```json
{"compact":{"exec":["/absolute/path/to/my-compactor"],"cwd":"/absolute/config"}}
```

The same mechanism replaces selected workers such as budget, memory or cache. It replaces all corresponding hook/tool bindings, not the core safety checks or native tools. A replacement can return a local summary to skip the summary-model request. Invalid output or worker failure never silently falls back to the built-in implementation. `init` preserves existing settings; regenerate and inspect desktop bindings after changes. Both startup scripts generate fresh temporary bindings automatically. See [configuration, protocol and the runnable Rust example](docs/compaction.md).

## Optional capabilities

- **Memory:** approved `memory_get/set/delete` tools; locked, atomic workspace state. Store only user-requested facts/preferences, never credentials.
- **Skills:** discover metadata under `.agents/skills` and `.i/skills`; load bodies/references on demand. Default roots cannot escape through symlinks. External roots require explicit configuration. Scripts are never autoexecuted.
- **File search (built in):** `glob` and `search`, with regex/glob/case options, filename-only results, bounded context and continuations. No search plugin required; see [batch file-tool interfaces](docs/file-tools.md).
- **Web:** configurable JSON GET/POST search and bounded public HTTP(S) text fetch; DNS/address checks, checked redirects, no environment proxy or JavaScript. Private-network access requires explicit trusted configuration.
- **Prompts:** embedded defaults; `config/prompts.json` selects the system prompt. The default compactor uses `config/compact.json`'s `prompt` path. Explicitly registered prompts hooks can still use `--system` / `--compact`.

Settings such as `compact.json`, `web.json`, `skills.json`, `prompts.json`, and `cache.json` are discovered under the platform configuration directory. See `examples/` and [the detailed contract](docs/plugins.md).

## Actual cache usage, not a promise

The cache module defaults to `observe`: it does not change provider caching controls. Explicit `openai` or `anthropic` modes add supported controls without replacing explicit request fields.

```sh
i --runtime cache-report -C /path/to/project
```

The ratio is **Σ cache-read input tokens / Σ all input tokens**. OpenAI cached tokens are already included in its input total; Anthropic input includes ordinary + cache-read + cache-creation, and creation is not a hit. DeepSeek uses returned hit/miss counts. Unknown usage and prepared requests without observed responses prevent target certification (`target_met: null`).

Statistics are durable, locked, and atomic; they contain no prompts, tool bodies, or credentials. `new`, `append`, `reset`, and `compact` classify request sequences, not actual hits. Cold requests and misses remain in the totals. The recent record/stream windows are bounded; cumulative counters remain.

**99% is a configurable target for a defined workload, not an achieved result or unconditional guarantee.** Cold starts, new content, compaction, expiration, and routing affect hits. Do not pad prompts or hide cold requests to inflate the metric.

## Low-cost DeepSeek smoke test

After setting a fresh `DEEPSEEK_TEST_API_KEY` privately:

```sh
cargo build --workspace --locked
work=$(mktemp -d)
./target/debug/i --config "$PWD/examples/deepseek.smoke.json" \
  -C "$work" --max-steps 1 'Reply only OK.'
./target/debug/i-plugins cache-report -C "$work" \
  --config "$PWD/plugins/smoke-cache.json"
```

This profile disables thinking, removes advertised tools, caps generation at 32 tokens, rejects unexpected tool calls before execution, and records separate observe-only smoke statistics. It still makes a billable provider request. The configured model can be overridden with `--model`. One request cannot establish a 99% hit rate. Parameter reference: [DeepSeek Chat Completions](https://api-docs.deepseek.com/api/create-chat-completion/).

For a thinking + image smoke test, use `examples/deepseek.thinking-vision.json` with `--image FILE -C IMAGE_DIRECTORY`; its policy permits up to 1,024 output tokens, still without tools. The smoke worker accepts explicit `--output-cap 1..1024`; its default remains 32.

## Network recovery, proxies and resume

API config accepts `"proxy":"socks5h://127.0.0.1:3067"` for an Xray SOCKS5 inbound with remote DNS, or `http://127.0.0.1:3067` for an HTTP CONNECT inbound. The port alone does not identify the protocol. Omit `proxy` to inherit proxy environment variables; `"proxy":""` forces direct connections. Web tools continue to disable proxies and validate actual destination addresses.

`network_retries` defaults to 5 (0..8). Transient model failures use bounded backoff and Retry-After within a 180-second network budget; local tools are not replayed and TLS verification remains enabled. A lost model response can still be billed, so retries can duplicate provider charges. Smoke examples disable retries. Failed attempts remain unknown in cache accounting, not fabricated hits.

To resume, select the same console `--session-name` and workspace, then enter a continuation task. The source launcher uses session `test`. Explicit core `--session FILE` also loads a saved checkpoint. There is no `--resume` flag; in-flight generation/unsaved queues are not restored. See [network settings, limits and recovery semantics](docs/network.md).

## Native release packages

Standard and min-size packages are generated under `dist/`. Size depends on the target, enabled features, compiler and compression; measure the actual release artifact rather than relying on historical numbers. The bundled build includes the optional console and workers. See [packaging instructions](docs/packaging.md) and [browser build details](docs/wasm.md).

```sh
# AArch64 size optimization; omit --outline on other architectures.
python3 scripts/package.py --profile min-size --outline
python3 scripts/check_package.py 'dist/*.tar.gz' 'dist/*.tar.xz'
```

Packaging/checking scripts are development utilities; distributed Rust executables do not require Python. For a short source-tree startup command after setting `DEEPSEEK_API_KEY`, use `./scripts/start.sh [workspace]`; it builds the Rust binaries and avoids clipboard line-break errors in long commands.

## Size and verification (historical native snapshot)

The table and validation counts below describe the earlier native snapshot, not the current cross-platform source. Current `src/` is 3,165 physical Rust lines excluding `src/tests.rs` and including packaging/completions and inline tests; runtime, vendor, JS and assets are additional. The old 1,024-line guard is explicitly ignored. See [current verification and known failures](docs/wasm.md).

| Scope | Physical Rust lines |
|---|---:|
| Core, including HTTP/retries and worker transport | **1,024 / 1,024** |
| Runtime source files, excluding dedicated test files (including inline tests) | **5,365** |
| Combined on that source-file basis | **6,389** |

The six provider source files contribute 1,076 of the runtime lines. Core plus those files is already 2,100 lines, before Responses/runtime support. A selected provider is required for model calls; the complete client is not claimed to fit in 1,024 lines.

The separately vendored globset dependency adds 3405 upstream Rust lines including tests/benches, previously fetched from the registry; its Rust sources are unchanged, only dependency features differ. See [provenance and integrity checks](vendor/globset/VENDORED.md). HTTP/retry/worker transport stays in the core; vendor encoding moved to replaceable providers as documented above.

Counts include blank lines/comments under committed rustfmt settings. Dedicated test modules are excluded; test entry declarations remain counted. The core test recursively checks all production Rust under `src/`, plus `build.rs` if present. Required core logic must not be hidden in plugins. `src/tools.json` is a required static schema asset, parsed once; it is disclosed separately rather than counted as Rust.

```sh
cargo fmt --all -- --check
cargo build --workspace --locked --features bundled
cargo test --workspace --locked --features bundled
cargo clippy --workspace --locked --all-targets --features bundled -- -D warnings
python3 scripts/check_tls.py --web
python3 scripts/check_console.py
python3 -m unittest discover -s examples -p 'test_plugins.py'
```

Current local validation: **188 bundled workspace Rust tests (182 without bundled)**, **295 upstream globset tests plus 5 doc tests**, **1 Rust replacement-example test** and **3 protocol-example tests**, plus pseudo-terminal editor and offline native-TLS rejection checks, on macOS with mocked APIs. Before the transport replacement, an earlier live DeepSeek `deepseek-flash` smoke request returned `OK` with exit 0 in 1.98 seconds end-to-end: 113 input tokens, 0 cache-read tokens, no tools, and a 32-token output cap. A separate thinking + PNG request (`reasoning_effort: low`, 1,024-token cap) correctly identified a red square on the left and a blue circle on the right in 2.13 seconds: 349 input tokens, 44 completion tokens including 33 reasoning tokens, 0 cache-read tokens and no tool calls. Reasoning content was present but not displayed. These individual requests verify connectivity/basic vision, not a warmed-workload cache target or worker speedup. CI is configured for Ubuntu/macOS/Windows, but Windows execution, a mobile implementation, and throughput benchmarks are not claimed. API success/error bodies are bounded to 48 MiB. Filesystem races, hard links, detached children, concurrent session writers, and approved code with user privileges remain outside any sandbox guarantee.
