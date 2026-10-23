#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
out="$(pwd)/target/browser-plugin-fixtures"
sh scripts/build_wasm_plugin.sh examples/wasm-word-count "$out"
cp target/wasm-plugin/wasm32-unknown-unknown/release/wasm_word_count.wasm "$out/word-count.wasm"
mkdir -p dist/plugins
cp "$out/example.word-count-1.0.0.hplugin.zip" dist/plugins/
sh scripts/build_wasm_plugin.sh examples/wasm-json-format dist/plugins
HARNESS_PLUGIN_FIXTURES="$out" cargo test -p i-wasm-package --test module emit_browser_fixtures
