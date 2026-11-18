#!/usr/bin/env bash
# Simple package verification script
set -euo pipefail

if [[ $# -lt 1 ]]; then
    echo "Usage: $(basename "$0") <archive.tar.gz|xz> [...]" >&2
    exit 1
fi

for archive in "$@"; do
    if [[ ! -f "$archive" ]]; then
        echo "Error: $archive not found" >&2
        exit 1
    fi

    echo "==> Verifying $(basename "$archive")..."

    # Create temp directory
    temp_dir=$(mktemp -d)
    trap "rm -rf $temp_dir" EXIT

    # Extract archive
    if [[ "$archive" == *.tar.gz ]]; then
        tar xzf "$archive" -C "$temp_dir"
    elif [[ "$archive" == *.tar.xz ]]; then
        tar xJf "$archive" -C "$temp_dir"
    else
        echo "Error: unsupported archive format" >&2
        exit 1
    fi

    # Find extracted directory
    bundle=$(find "$temp_dir" -mindepth 1 -maxdepth 1 -type d | head -1)
    if [[ -z "$bundle" ]]; then
        echo "Error: archive is empty" >&2
        exit 1
    fi

    # Verify SHA256SUMS
    echo "  Checking SHA256SUMS..."
    (cd "$bundle" && shasum -a 256 -c SHA256SUMS 2>/dev/null || sha256sum -c SHA256SUMS 2>/dev/null) || {
        echo "Error: SHA256SUMS verification failed" >&2
        exit 1
    }

    # Verify BUILD.json
    echo "  Checking BUILD.json..."
    if ! grep -q '"layout": "bundled"' "$bundle/BUILD.json"; then
        echo "Error: BUILD.json layout is not 'bundled'" >&2
        exit 1
    fi

    # Verify single binary
    echo "  Checking binary count..."
    binary_count=$(ls -1 "$bundle/bin" | wc -l | tr -d ' ')
    if [[ "$binary_count" != "1" ]]; then
        echo "Error: expected 1 binary, found $binary_count" >&2
        exit 1
    fi

    # Test binary execution
    echo "  Testing binary..."
    binary="$bundle/bin/i"
    if [[ ! -x "$binary" ]]; then
        echo "Error: binary is not executable" >&2
        exit 1
    fi

    if ! "$binary" --version >/dev/null 2>&1; then
        echo "Error: binary failed to execute" >&2
        exit 1
    fi

    version=$("$binary" --version 2>&1 | head -1)
    echo "    Version: $version"

    # Verify archive checksum
    sha_file="${archive}.sha256"
    if [[ -f "$sha_file" ]]; then
        echo "  Checking archive checksum..."
        expected=$(awk '{print $1}' "$sha_file")
        if command -v shasum >/dev/null 2>&1; then
            actual=$(shasum -a 256 "$archive" | awk '{print $1}')
        else
            actual=$(sha256sum "$archive" | awk '{print $1}')
        fi
        if [[ "$expected" != "$actual" ]]; then
            echo "Error: archive checksum mismatch" >&2
            echo "  Expected: $expected" >&2
            echo "  Actual:   $actual" >&2
            exit 1
        fi
    fi

    echo "==> PASS: $(basename "$archive")"
    echo ""
done
