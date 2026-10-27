# Static site tooling

This independent Rust crate replaces the site, translation, CORS-extension and
Safari-project build scripts. It does not require changes to the application
workspace or enable application runtime features.

Run from the repository root:

```sh
cargo run --locked --manifest-path tools/site-tool/Cargo.toml -- translations
cargo run --locked --manifest-path tools/site-tool/Cargo.toml -- translations --check
cargo run --locked --manifest-path tools/site-tool/Cargo.toml -- cors-extension
cargo run --locked --manifest-path tools/site-tool/Cargo.toml -- cors-safari
cargo run --locked --manifest-path tools/site-tool/Cargo.toml -- cors-safari --build
cargo run --locked --manifest-path tools/site-tool/Cargo.toml -- site --origin https://i.lninl.com
cargo run --locked --manifest-path tools/site-tool/Cargo.toml -- serve --directory dist/site --port 8090
```

`--root PATH` selects a separate source tree for fixtures. `site --output PATH`
selects an output directory; an existing directory is replaced only when it
contains the generated-site marker. Builds validate translation freshness and
the PWA asset hashes, and publish only the three verified extension downloads.

Generate translations before rebuilding PWA assets. The generated translation
banner now names this crate; all other translation output is preserved. CORS
archives use sorted entries, fixed timestamps, fixed permissions, and checksums.
Safari conversion/build requires the platform's `xcrun`/`xcodebuild`; neither a
successful archive build nor the `runtime_verified: false` receipt claims a
Safari runtime verification.

The preview server binds only to `127.0.0.1`, accepts GET/HEAD, rejects traversal
and symlink escapes, and serves the WASM MIME type. It performs no builds or
model requests.

```sh
cargo test --locked --manifest-path tools/site-tool/Cargo.toml
cargo clippy --locked --manifest-path tools/site-tool/Cargo.toml --all-targets -- -D warnings
```

Tests use temporary source/output trees, preserve the former site and catalog
assertions, and check malformed metadata, checksum failures, symlinks, duplicate
JSON keys, translation boundaries, reproducibility, and stale generated files.
Node is used only to exercise the existing frontend PNG rasterizer in memory;
the tooling itself is Rust, and tests do not modify served assets.
