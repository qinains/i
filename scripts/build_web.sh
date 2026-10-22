#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
wasm-pack build --target web --out-dir web/pkg --out-name i --no-opt --no-default-features --features wasm "$@"
