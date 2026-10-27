# Contributing

Source: https://github.com/qinains/i · Documentation: https://i.lninl.com/docs/

## Start here

- [System overview](docs/system-overview.md)
- [Build your first plugin](docs/plugin-development.md)
- [Browser WASM plugin reference](docs/wasm-plugin-reference.md)
- [Native worker contract](docs/plugins.md)
- [Security reporting](SECURITY.md)

Rust 1.91+ is the native development prerequisite; frontend builds/tests also use Node.js 22+.
Repository automation and fixtures are Rust and do not require Python. Do not use a real
API key for unit/browser tests. Keep model traffic mocked unless a maintainer
explicitly authorizes a paid integration test.

## Development

```sh
cargo build --workspace
cargo test --locked --manifest-path tools/repo-tool/Cargo.toml
cargo test --locked --manifest-path tools/site-tool/Cargo.toml
sh scripts/build_web.sh
cd web
npm ci
npx playwright install chromium webkit
npm test
npm run test:browser -- tests/settings.spec.mjs tests/pending.spec.mjs
```

Run relevant Rust tests as well. For a plugin change:

```sh
cargo test -p i-wasm-sdk -p i-wasm-protocol -p i-wasm-package
sh scripts/build_plugin_fixtures.sh
cd web
npm run test:browser -- tests/plugin-agent.spec.mjs tests/plugin-broker.spec.mjs
```

Run `npm run test:browser` for the complete Chromium/WebKit suite; see
[browser verification notes](docs/wasm.md) for coverage and limitations. Report
exact commands and failures; do not claim a targeted run is the full suite.

Pull requests run the complete Chromium/WebKit suite before the website can deploy.
Browser failures retain traces and screenshots for seven days. These artifacts use
synthetic test accounts only; never reproduce a real key or conversation in CI.
The Chromium crash test launches a separate persistent profile and kills only its
own browser process before an OPFS write closes. It checks committed bytes and
subsequent writes after restart, not OS power-loss durability or storage eviction.

Security checks scan Git history and the checkout with Gitleaks, and check locked
Rust/JavaScript dependencies against vulnerability databases on PRs and weekly.
Website and native release builds also scan their output and nested download
archives before uploading artifacts. Run locally:

```sh
cargo audit --file Cargo.lock
npm audit --prefix web --audit-level=high
gitleaks git --redact --config .gitleaks.toml --log-opts="--all" .
gitleaks dir --redact --config .gitleaks.toml --max-archive-depth 3 dist/site
```

Install Gitleaks 8.30.0 and cargo-audit 0.22.2 for parity with CI. Scan only this
repository and synthetic build outputs, not personal browser or credential stores.
Do not add blanket exclusions to make security checks pass. The JWT exception in
`.gitleaks.toml` is restricted to the public-key, synthetic signature fixture.

## Pull requests

1. Fork the repository, create a focused branch from the default branch.
2. Describe the user-visible problem and the selected behavior before code.
3. Add a regression test that fails before the fix. For protocol changes, test
   malformed messages, cancellation, permissions, limits and partial effects.
4. Preserve unrelated edits. Avoid blanket formatting and generated artifacts.
5. Update user-facing docs and examples in the same PR.
6. Fill in the PR template: summary, interface changes, test evidence, security,
   compatibility, and known gaps. Screenshots help with UI changes.

Never commit `.env`, real keys, SSH material, browser profiles, local harness
configuration, OPFS/IndexedDB exports, real conversations or private endpoints.
Use unmistakable placeholders and synthetic fixtures. Run the release scanner
against the exact export before publishing. Do not attach raw network headers.

## Plugin submissions

Supply source, MIT-compatible licensing, a bounded manifest/schema, minimal
permissions, reproducible build commands, a synthetic test input/output and
failure behavior. Native subprocesses are trusted local programs, not a sandbox.
Browser plugins must use the WASM SDK and approved host operations, not DOM,
network, shell or native filesystem access. Installation is not permission to
enable a plugin silently in every workspace.

Contributions are licensed under the repository's MIT license. Preserve upstream
copyright and license notices; MIT for this project does not relicense vendored
dependencies or third-party assets.
