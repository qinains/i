# Contributing

Source: https://github.com/qinains/i · Documentation: https://i.lninl.com/docs/

## Start here

- [System overview](docs/system-overview.md)
- [Build your first plugin](docs/plugin-development.md)
- [Browser WASM plugin reference](docs/wasm-plugin-reference.md)
- [Native worker contract](docs/plugins.md)
- [Security reporting](SECURITY.md)

Rust 1.91+, Node.js and Python 3 are development prerequisites. Do not use a real
API key for unit/browser tests. Keep model traffic mocked unless a maintainer
explicitly authorizes a paid integration test.

## Development

```sh
cargo build --workspace
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

The complete browser suite has historical failing expectations documented in
[browser verification notes](docs/wasm.md). Report exact commands and failures;
do not hide failures or claim that a targeted run is the full suite.

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
