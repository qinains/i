# Replaceable compaction and built-in worker bindings

Compaction is an optional **`compact` worker module**, separate from `budget`.
The built-in module is a default policy, not a required implementation. A
replacement can choose retention, prompts, image handling and postprocessing,
or return a locally produced summary without a summary-model request.

## Enable or replace

- `standard` / `full` include both budgeting and compaction.
- `compact` includes budget + compact; `responses-compact` also includes Responses.
- `budget` and `responses-budget` now provide budgeting **without** compaction.
- Portable `plugins.json` can use `"disabled":["compact"]` to remove compaction
  while retaining budget checks. No compactor means no automatic fallback;
  reaching the hard context budget produces an error before model HTTP.

For the built-in policy, `config_dir/compact.json` accepts:

```json
{"enabled":true,"keep_recent":4,"images":"keep","prompt":"prompts/compact.txt"}
```

All fields are optional. `keep_recent` is 1..10000 and is a minimum; complete
recent tool batches can retain more. The module follows the budget's `compact`
recommendation. `prompt` is relative to this configuration file; absent it, an
embedded default is used (or a prompt supplied by an earlier hook). Prompts are
bounded to 32000 characters and support `{{workspace}}`. `images` is
`keep`, `omit` or `describe`; absent it, the API budget's model-specific `images`
setting is used, otherwise `keep`. Budget image planning still requires
`image_tokens`. `enabled:false` disables this policy without disabling budgeting.

**Replace an implementation** in the host-specific
`~/.i/config/bindings.json` (or `$I_CONFIG_DIR/bindings.json`):

```json
{
  "compact": {
    "exec": ["/absolute/path/to/my-compactor", "--my-option"],
    "cwd": "/absolute/path/to/plugin-config"
  }
}
```

`cwd` is optional; ordinary generated workers default to the platform config
directory. Executable and explicit cwd paths must be absolute. `exec` is an argv
array, never a shell command. Unknown module names, invalid bindings and worker
failures are errors: **there is no silent fallback to the built-in module**.
Do not put credentials in argv or JSON settings.

This mechanism also replaces selected built-in workers such as `budget`,
`memory`, `cache`, `search`, `skills`, `prompts`, `web` and `responses`. Every
selected hook/tool entry belonging to that module receives the same binding;
tool names/schemas and lifecycle event declarations remain unchanged. A module
replacement must implement those interfaces, not just one tool. These bindings
do not replace the mandatory core tools, core safety checks, or the console/logo
frontend code. The console's injected steering worker is also replaceable; it
receives an appended `--queue PATH` argument and must implement that queue
contract. Profile disabling does not remove this console dependency.

Keep executable paths out of portable `plugins.json`. After changing bindings
or upgrading from the former budget-owned compaction implementation, generate
and inspect a new desktop manifest:

```sh
i --runtime manifest standard > /tmp/i-desktop.json
# Inspect it, then replace config_dir/desktop.json with this file.
```

`init` preserves existing files, including customized bindings/manifests. The
source and packaged `start.sh` launchers generate fresh temporary manifests, so
they pick up the new bindings automatically. Explicit hand-written manifests
remain authoritative rather than being rewritten. Removing a binding and
regenerating restores the default implementation.

## Worker contract

Use the existing persistent NDJSON protocol with matching unsigned request IDs,
`event`, `workspace` and object `data`. Return a complete updated `data` object,
preserving unrelated fields. Transport limits, the 15-second worker deadline,
environment isolation, failure eviction and **no worker replay** are unchanged.
These are trusted programs, not sandboxed code.

1. **`compact_plan`** receives budget output (including recommendation, estimates,
   model/policy metadata) plus the authoritative full `history`. Budget providers
   must preserve their input metadata. Core initializes `cut` to null, so an
   absent compactor cannot accidentally inherit a cutoff from a budget provider.
   Return `cut:null` to decline, or an unsigned absolute history index `cut` and
   optional opaque `state`. A replacement may choose its own trigger rather than
   following the default recommendation.
2. Core checks **`2 <= cut < history.len()`** and that `history[cut].role` is
   `user` or `assistant`. The already-validated history cannot be cut inside a
   tool-result batch. Core also requires a comparable pre-compaction estimate.
3. **`before_compact`** receives `messages = history[1..cut]`, `cut`, `state`,
   the configured `model` and budget `policy`. The system message and recent
   suffix are not delegated for rewriting.
   - For model-assisted summarization, return `prompt`, prepared `messages`,
     optional `summary_images` and `retained_images`. Strip private adapter
     state/base64 from the transcript as appropriate. The core supplies the
     untrusted-transcript wrapper and makes the model request itself.
   - Alternatively return a **nonempty string `summary`**, plus optional
     `retained_images`, to skip the summary-model request. Presence of `summary`
     selects this route; malformed/empty values fail rather than falling back.
4. **`after_compact`** receives `summary` and the `state` returned by preparation.
   It can postprocess the summary. Final output must still be a nonempty string.
5. Core constructs an untrusted user-summary message after the existing system
   message, appends the untouched recent suffix, then runs
   `before_compact_commit` for reference snapshot restoration and validates the
   candidate. It must fit the budget and reduce comparable content estimates.
   On success the old history is saved to `.precompact.json` and the candidate is
   saved through the normal checkpoint hooks. A replacement does not get to
   bypass those checks. Preparation/worker/validation failures do not commit a
   compressed candidate or replay tools; the earlier user/tool checkpoint remains.

The core owns API credentials, transport, retries, tool-call pairing, history
validation and checkpoint execution. A model-generated summary must finish
normally without tool calls; summary requests are budget-checked before HTTP.
The plugin never needs the model key. Local compaction skips that model request,
not the next normal agent request. Missing/comparable budget data cannot be
invented by the compactor: the baseline is captured before planning.

## Runnable Rust replacement example

```sh
cargo build --release --locked --manifest-path examples/rust-plugin/Cargo.toml
```

Bind `compact.exec` to the absolute path of
`examples/rust-plugin/target/release/i-plugin-example` (add `.exe` on Windows),
with `"--compact"` as the next argv element. This uses the same framed worker
protocol and existing serde_json dependency; no new production crate is added.

The demonstration returns **local extractive notes**, capped at 2048 UTF-8 bytes,
and deliberately omits old image attachments and private adapter fields. It is
lossy and **not a semantically equivalent replacement for model summarization**.
The host may reject it if it does not reduce the history or fit the budget. It
is an implementation starting point, not a promise of compaction quality.

Tests cover replacement through generated bindings, local-summary request counts,
state roundtrips, persistent reuse, invalid cuts, tool-batch splits, malformed or
oversized/nonreducing summaries, worker failure without fallback, and budgeting
without a compactor. All use local fixtures, not paid model requests.
