#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
wasm-pack build plugins/wasm-package --target web --out-dir "$(pwd)/web/plugin-pkg" --out-name validator --profile "${WASM_PROFILE:-min-size}" --no-opt --features web
