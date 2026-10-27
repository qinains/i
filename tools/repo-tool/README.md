# Repository audit and export tool

This independent Rust workspace replaces the license-notice, release-export and
globset policy scripts. It does not execute dependency source code or use an
external interpreter, GitHub CLI, curl, or Git executable.

```sh
cargo test --locked --manifest-path tools/repo-tool/Cargo.toml
cargo run --locked --manifest-path tools/repo-tool/Cargo.toml -- licenses
cargo run --locked --manifest-path tools/repo-tool/Cargo.toml -- licenses --check
cargo run --locked --manifest-path tools/repo-tool/Cargo.toml -- check-globset
cargo run --locked --manifest-path tools/repo-tool/Cargo.toml -- check-no-python
cargo run --locked --manifest-path tools/repo-tool/Cargo.toml -- export-release
cargo run --locked --manifest-path tools/repo-tool/Cargo.toml -- bench-search BEFORE AFTER --runs 7
```

From the repository root, use `--target-dir target`, or set `CARGO_TARGET_DIR`
to the absolute repository target directory, to share compatible artifacts across
independent workspaces. Feature/profile differences can still require rebuilding.

`check-no-python` rejects repository interpreter source/bytecode, retired script
references, and direct shell/workflow/Node/Rust interpreter entrypoints while
allowing Python language descriptions and user-authored external plugins. It
skips generated dependency/build/cache trees. This focused dependency guard is
not a general parser or proof against dynamically constructed subprocess code.

License collection uses `cargo metadata --all-features --locked --offline`,
checks complete package identities against each selected Cargo.lock, and retains
build, test, optional and target-specific dependencies. Default collection also
includes the separately locked workspaces under `tools/*`. Run `cargo fetch --locked` for
each workspace beforehand when a cache is incomplete. It never silently skips
unavailable sources or missing license text.

For an independent plugin, use `licenses --manifest-path PATH --output PATH`.
An explicit manifest always requires a separate output, so a plugin subset cannot
overwrite the main inventory accidentally. `--include-license PATH` prepends the
plugin's own license and requires an explicit manifest.

`licenses --fetch-missing` is the only network operation: it fetches root notices
from explicitly reviewed public GitHub repositories at the crate's packaged
`.cargo_vcs_info.json` revision. Every downloaded Git blob is verified by its
SHA-1 object ID, then stored by SHA-256 under `vendor/licenses`. Normal collection
and `--check` only read cached Cargo sources and checksum-verified supplements.
For `asn1-rs-impl 0.2.0`, the reviewed upstream is `rusticata/asn1-rs`, revision
`a20e5f7319c896737ad0f2557037817b91ad854f`; the subcrate inherits the repository's
root MIT and Apache notices, which were absent from the published subcrate.

Export creates a fresh `dist/release-source` snapshot, never replaces an existing
one, rejects symlinks, excludes credentials and generated artifacts, and retains
source tests, notices, scanner policy and Rust tooling. The adjacent inventory
records SHA-256 hashes of the actual copied bytes. It is not an approval to
publish the snapshot.

The offline search comparison preserves the eight-file, 6,400,000-byte fixture,
per-run worker process, correctness checks, 30-second timeout, individual samples
and median. It measures process startup and filesystem reads, not general search
throughput. Pass bundled binaries supporting `--runtime search`; core-only `i`
does not provide that worker command.
