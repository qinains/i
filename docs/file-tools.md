# Efficient file tools

The browser and native agent share these built-in tools. The model receives their
schemas automatically. They do not need a plugin or shell.

This is a new interface, not a compatibility layer: `read.path/offset/limit` and
`edit.old/new` are replaced by the batch forms below. Existing conversation history
can still be displayed, but old tool arguments must not be replayed.

## Recommended workflow

1. Discover paths with `glob`, or locate code directly with `search`.
2. Read the relevant ranges from multiple files in one `read`.
3. Put all same-file replacements in one `edit`.
4. Batch independent tool calls in the same assistant response. Do not reread
   unchanged files or repeat their contents in the final answer.

Searching first reduces irrelevant content in model context; batching reduces
model round trips. Neither guarantees a particular token saving with a live
model: model behavior, schema overhead and task size also matter.

## Examples

Find all source and test files without listing each directory:

```json
{"patterns":["src/**/*.rs","tests/**/*.rs"]}
```

Search for relevant code (`search`):

```json
{"pattern":"configure|validate","regex":true,"glob":"**/*.rs","context_lines":1,"limit":20}
```

Use `mode: "files"` when only matching filenames are needed. Search is literal
and case-sensitive by default. Regex is line-based Rust regex, not JavaScript
regex; lookaround and backreferences are not supported. Snippets are bounded
around the first match on each line, with explicit truncation markers.

Batch reads (`read`):

```json
{"files":[{"path":"src/app.rs","start_line":20,"end_line":50},{"path":"tests/app.rs","start_line":10,"end_line":30}],"max_bytes":12000}
```

Lines are 1-based; end is inclusive. Default is 200 lines per range, maximum
1000 lines per range and 16 ranges per call. Repeated ranges of one file reuse
the same in-call read, not a persistent cache. Per-file failures are returned
alongside successful ranges. Invalid ranges/paths reject the whole request.

Batch edits (`edit`):

```json
{"path":"src/app.rs","edits":[{"old":"return false;","new":"return true;"},{"old":"old label","new":"new label","line_number":42}]}
```

Up to 64 replacements run sequentially against the evolving text. Each `old`
must match exactly once; `line_number` disambiguates a single-line occurrence
in the current text. All replacements and size limits are validated in memory
before one write. An invalid replacement leaves the file unchanged. The file
is reread before committing to detect concurrent changes. Line endings are
preserved. This is single-file batching, not a multi-file transaction, and the
concurrent-change check is not an OS-level compare-and-swap.

## Output and scan bounds

- `list`, `glob`, `search`, `read` return compact JSON inside `output`, containing
  `results`, `truncated`, `next`, and `skipped`.
- Output budget defaults to 12000 bytes **shared by all results in the call**.
  `max_bytes` is 1024..64000, also capped by the host's output allowance.
  This is a byte budget, not a tokenizer-specific token estimate.
- `list`/`glob`: default 100 results, maximum 500. Pass the returned numeric
  `next` as `cursor` to continue.
- `search`: default 50 hits, maximum 200; 0..3 context lines. Pass the returned
  `{file_index,line}` as `cursor`. `line` in this opaque cursor is zero-based;
  displayed result line numbers are one-based.
- `read`: `next` identifies the original `files` index and the first unread
  `start_line`. Retry that range and remaining files; preserve its original end.
  A single line too large for the budget returns an explicit error and a
  continuation, not an apparently complete but clipped line. Increase the
  budget or use `search` for a bounded snippet.
- Cursors assume unchanged files/directories and unchanged query options;
  restart discovery if files change. No workspace-wide index/cache is maintained.
- Recursive tools visit at most 10000 entries. Larger trees fail explicitly:
  narrow `path`. `path` limits traversal; glob patterns still match full
  workspace-relative paths, including the directory prefix.
- By default recursion skips `.git`, `node_modules`, `target`, `.i` directories.
  Set `include_ignored: true` to traverse them. This is a fixed directory list,
  not a `.gitignore` interpreter. An explicitly selected root is still searched.
- Search skips binary, non-UTF-8 and oversize files and reports their count.
  It reads at most 64 MiB per page based on file sizes, with a continuation for
  the next file. Read batches retain at most 64 MiB of text. Native/browser
  per-file limits and path confinement remain enforced.
- A small result budget does not make disk scans free: a selective search can
  still scan every eligible file. Narrow `path`/`glob` when possible.

## Permissions and native manifests

Built-in `glob`, `search`, `list`, `read`, `view_image` are read-only and do not
ask for modification approval by default. Native approval hooks still see
every call and can deny or require confirmation. `edit`/`write` keep their
normal approval and persistence checks. Browser shell remains disabled.

Native `full`/`standard` manifests no longer advertise the redundant external
`search`/`find_files` tools. `glob` replaces file discovery; built-in `search`
uses `pattern`, not the old plugin's `query`. Regenerate old static manifests
and remove custom entries colliding with built-in names. The `search` profile
now has no extra tools because search is already built in.
