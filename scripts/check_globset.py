#!/usr/bin/env python3
"""Verify globset stays an unmodified, checksum-locked crates.io dependency."""
from pathlib import Path
import re
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def main():
    workspace = tomllib.loads((ROOT / 'Cargo.toml').read_text())
    runtime = tomllib.loads((ROOT / 'plugins/runtime/Cargo.toml').read_text())
    if workspace['dependencies']['globset'] != {'version': '=0.4.20', 'default-features': False} or runtime['dependencies']['globset'] != '=0.4.20':
        raise SystemExit('Review and update the official globset pin explicitly')
    if 'globset' in workspace.get('patch', {}).get('crates-io', {}) or any(path.is_file() or path.is_symlink() for path in (ROOT / 'vendor/globset').rglob('*')):
        raise SystemExit('A local globset override is not allowed')
    packages = tomllib.loads((ROOT / 'Cargo.lock').read_text())['package']
    entries = [package for package in packages if package['name'] == 'globset']
    if len(entries) != 1 or entries[0]['version'] != '0.4.20' or entries[0].get('source') != 'registry+https://github.com/rust-lang/crates.io-index' or not re.fullmatch('[a-f0-9]{64}', entries[0].get('checksum', '')):
        raise SystemExit('globset must have one checksum-locked crates.io source')
    print('PASS: official globset 0.4.20, locked registry checksum, no local patch')


if __name__ == '__main__':
    main()
