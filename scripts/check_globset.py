#!/usr/bin/env python3
"""Check that the globset patch changes dependency features only, not upstream Rust."""
import hashlib
import json
from pathlib import Path
import tomllib
import subprocess
import sys
import shutil
import tempfile

ROOT = Path(__file__).resolve().parents[1]
VENDOR = ROOT / 'vendor/globset'


def main():
    hashes = json.loads((VENDOR / 'UPSTREAM-SHA256.json').read_text())
    for name, expected in hashes.items():
        actual = hashlib.sha256((VENDOR / name).read_bytes()).hexdigest()
        if actual != expected:
            raise SystemExit(f'Upstream globset file changed: {name}')
    rust_files = {str(p.relative_to(VENDOR)).replace('\\', '/')
                  for folder in ('src', 'benches') for p in (VENDOR / folder).rglob('*.rs')}
    if rust_files != {name for name in hashes if name.endswith('.rs')}:
        raise SystemExit('Unexpected/missing vendored Rust source')
    upstream = tomllib.loads((VENDOR / 'Cargo.toml.upstream').read_text())
    current = tomllib.loads((VENDOR / 'Cargo.toml').read_text())
    upstream['dependencies']['regex-automata']['features'] = [
        'std', 'perf-literal', 'syntax', 'meta', 'nfa-pikevm']
    if current != upstream:
        raise SystemExit('Globset manifest differs beyond the reviewed feature change')
    runtime = tomllib.loads((ROOT / 'plugins/runtime/Cargo.toml').read_text())
    workspace = tomllib.loads((ROOT / 'Cargo.toml').read_text())
    if runtime['dependencies']['globset'] != '=0.4.20':
        raise SystemExit('Globset must stay pinned until the patch is re-audited')
    if workspace['patch']['crates-io']['globset'] != {'path': 'vendor/globset'}:
        raise SystemExit('Globset patch is not selected')
    print('PASS: upstream globset Rust/licenses unchanged; only reviewed backend features differ')
    if '--test' in sys.argv[1:]:
        # Keep the standalone upstream crate outside any enclosing workspace.
        with tempfile.TemporaryDirectory(prefix='i-globset-test-') as temporary:
            crate = Path(temporary) / 'globset'
            shutil.copytree(VENDOR, crate)
            subprocess.run(['cargo', 'test', '--manifest-path', str(crate / 'Cargo.toml'),
                            '--target-dir', str(ROOT / 'target/vendor-check'), '--locked',
                            '--features', 'serde1'], check=True, cwd=crate)
    if '--graph' in sys.argv[1:]:
        version = subprocess.check_output(['rustc', '-vV'], text=True)
        host = next(line.split(': ', 1)[1] for line in version.splitlines() if line.startswith('host: '))
        metadata = json.loads(subprocess.check_output(
            ['cargo', 'metadata', '--locked', '--offline', '--features', 'bundled', '--format-version', '1',
             '--filter-platform', host],
            cwd=ROOT, text=True))
        ids = {p['id'] for p in metadata['packages'] if p['name'] == 'regex-automata'}
        features = {f for n in metadata['resolve']['nodes'] if n['id'] in ids for f in n['features']}
        # The shared workspace search tool deliberately enables regex/perf.
        # Check the exact reviewed union rather than claiming globset alone
        # determines every consumer's backend features.
        expected = {'alloc', 'dfa-onepass', 'hybrid', 'meta', 'nfa-backtrack', 'nfa-pikevm', 'nfa-thompson', 'perf-inline', 'perf-literal', 'perf-literal-multisubstring', 'perf-literal-substring', 'std', 'syntax', 'unicode', 'unicode-age', 'unicode-bool', 'unicode-case', 'unicode-gencat', 'unicode-perl', 'unicode-script', 'unicode-segment', 'unicode-word-boundary'}
        if not ids or features != expected:
            raise SystemExit(f'Unexpected regex backend feature union: {sorted(features)}')
        print('PASS: resolved graph matches the reviewed shared regex/perf feature union')


if __name__ == '__main__':
    main()
