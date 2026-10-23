# Globset 0.4.20 feature-only patch

Origin: crates.io `globset` 0.4.20, upstream https://github.com/BurntSushi/ripgrep/tree/master/crates/globset . Original license: Unlicense OR MIT. The upstream licenses, README, Cargo.toml.orig, benches and **all Rust sources including upstream tests are preserved byte-for-byte**. `Cargo.toml.upstream` is the original normalized crates.io manifest.

The only upstream manifest change is the regex-automata feature list:

```text
before: std, perf,         syntax, meta, nfa,         hybrid
after:  std, perf-literal, syntax, meta, nfa-pikevm
```

No parser, matching, error-handling or platform source was rewritten. Literal prefiltering is deliberately retained: removing it caused a large slowdown in a local literal-search fixture. Full Unicode support is still selected by the application's regex dependency. Complex regex workloads can nevertheless be slower without the other engines; this is a size/performance tradeoff, not a universal speed claim.

The dependency is pinned to `=0.4.20` and selected with the root `[patch.crates-io]`. Upstream has no feature switch for this exact backend selection. This copy must be deliberately re-audited on updates; do not let an automatic version update bypass the patch. Ideally an upstream feature split can replace it in future.

## Validation

```sh
python3 scripts/check_globset.py
# After the native project's dependencies have been fetched/built:
python3 scripts/check_globset.py --graph
cargo test --manifest-path vendor/globset/Cargo.toml --locked --features serde1
cargo test --workspace --locked --features bundled
# Compare separately saved bundled binaries on a generated offline fixture:
python3 scripts/bench_search.py /path/to/before/i /path/to/after/i
```

`UPSTREAM-SHA256.json` verifies the unchanged copied files. The checker also compares the complete parsed manifests, permits only the reviewed feature delta, and optionally checks Cargo's unified feature graph for accidental re-enablement. `.gitattributes` prevents line-ending conversion from changing audited bytes on Windows. The independent Cargo.lock pins upstream test dependencies; it is not a runtime requirement.

Locally, 295 upstream tests and 5 doc tests passed with serde support enabled. Platform-specific behavior still requires the configured CI; only macOS arm64 was executed locally. Source tests include complex glob patterns and byte-oriented filename behavior; they do not justify narrowing glob syntax.

This directory contains **3405 upstream Rust lines including upstream tests and benches**. It is existing third-party dependency source, not relocated first-party core code. The first-party mandatory core remains 1024 lines and its existing guard is unchanged. Published packages include the globset MIT notice, not the vendored source/test tree. This is not a complete dependency-license audit.
