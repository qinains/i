#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
wasm-pack build --target web --out-dir web/pkg --out-name i --no-opt --no-default-features --features wasm "$@"
for bridge in web/pkg/snippets/*/web/platform.js; do
    cp web/sse.js "$(dirname "$bridge")/sse.js"
done
