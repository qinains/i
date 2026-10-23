# Browser JSON formatter plugin

Build both browser-test example packages from the repository root:

```sh
sh scripts/build_plugin_fixtures.sh
```

Outputs:

- `dist/plugins/example.word-count-1.0.0.hplugin.zip`
- `dist/plugins/example.json-format-1.0.0.hplugin.zip`

In the browser, open Settings > Plugins. Choose each ZIP, review its tool and
permissions, install, then explicitly enable it for the current persistent
workspace. Installation alone does not grant access.

Word count requires read access. JSON formatter requires read/write access, reads
the JSON at `path`, and writes indented JSON plus a newline to `output_path`.
Normal host approval still applies to writes; invalid JSON does not produce an
output file. Neither plugin requests network access or private KV storage.

Example tool arguments for `format_json`:

```json
{"path":"input.json","output_path":"formatted.json"}
```

Test unit behavior and the actual two-plugin browser installation flow:

```sh
cargo test --manifest-path examples/wasm-json-format/Cargo.toml
cd web
npm run test:browser -- tests/two-plugins.spec.mjs
```

The browser test imports the built ZIPs through the actual settings UI, enables
both, approves model calls and the formatter's file write, checks persisted
formatted output after reload, verifies disabling one does not disable the other,
and exports a workspace ZIP without plugin program files. The model endpoint is
mocked; plugin execution is real Rust WASM in dedicated workers.
