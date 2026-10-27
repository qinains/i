#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo run --quiet --locked --manifest-path tools/site-tool/Cargo.toml --target-dir target -- translations --check
wasm-pack build --target web --out-dir web/pkg --out-name i --profile "${WASM_PROFILE:-min-size}" --no-opt --no-default-features --features wasm "$@"
for bridge in web/pkg/snippets/*/web/platform.js; do
    cp web/sse.js "$(dirname "$bridge")/sse.js"
done
sh scripts/build_plugin_validator.sh
sh scripts/build_wasm_plugin.sh plugins/wasm-prompts web/plugin-pkg
node scripts/build_pwa.mjs
