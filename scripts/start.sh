#!/bin/sh
# Development launcher only; the agent, plugins, and console are Rust.
set -eu
if [ "${1:-}" = "--help" ]; then
    printf 'Usage: ./scripts/start.sh [workspace]\nRequires DEEPSEEK_API_KEY; uses the DeepSeek example and desktop plugins.\n'
    exit 0
fi
if [ "$#" -gt 1 ]; then
    printf 'Usage: ./scripts/start.sh [workspace]\n' >&2
    exit 2
fi
if [ -z "${DEEPSEEK_API_KEY:-}" ]; then
    printf 'Set DEEPSEEK_API_KEY in this terminal first (do not put the key in this script).\n' >&2
    exit 1
fi
repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
workspace=${1:-"$PWD"}
if [ ! -d "$workspace" ]; then
    printf 'Workspace directory does not exist: %s\n' "$workspace" >&2
    exit 1
fi
cargo build --manifest-path "$repo/Cargo.toml" --target-dir "$repo/target" --features bundled --bin i --locked
"$repo/target/debug/i" --runtime init
manifest=$(mktemp "${TMPDIR:-/tmp}/i-plugins.XXXXXX")
trap 'rm -f "$manifest"' 0
trap 'exit 130' 2
trap 'exit 143' 15
"$repo/target/debug/i" --runtime manifest standard > "$manifest"
"$repo/target/debug/i" --runtime console \
    --config="${I_API_CONFIG:-$repo/examples/api.openai.json}" \
    --plugins="$manifest" \
    --dir="$workspace" \
    --session-name=test
