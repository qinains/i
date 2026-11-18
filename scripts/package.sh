#!/usr/bin/env bash
# Wrapper script for the Rust-based packaging tool
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PACKAGE_TOOL="$ROOT/target/debug/i-package"

# Build the packaging tool if needed
if [[ ! -f "$PACKAGE_TOOL" ]]; then
    echo "==> Building packaging tool..."
    cargo build --features packaging --bin i-package --manifest-path "$ROOT/Cargo.toml"
fi

# Pass all arguments to the Rust tool
exec "$PACKAGE_TOOL" "$@"
