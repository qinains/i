#!/usr/bin/env python3
"""Small offline literal-search comparison, not a general throughput benchmark."""
import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binaries', nargs='+', type=Path)
    parser.add_argument('--runs', type=int, default=3)
    args = parser.parse_args()
    if not 1 <= args.runs <= 100:
        parser.error('--runs must be 1..100')
    with tempfile.TemporaryDirectory(prefix='i-search-bench-') as temporary:
        root = Path(temporary)
        workspace = root / 'project'
        workspace.mkdir()
        content = b'fn main() { let counter = 42; }\n' * 25000
        for index in range(8):
            (workspace / f'{index}.txt').write_bytes(content)
        environment = os.environ.copy()
        for kind in ('CONFIG', 'DATA', 'CACHE', 'RUNTIME'):
            environment[f'I_{kind}_DIR'] = str(root / kind.lower())
        frame = {'id': 1, 'event': 'tool', 'workspace': str(workspace),
                 'data': {'name': 'search', 'arguments': {'query': 'absentneedle'}}}
        for binary in args.binaries:
            samples = []
            for _ in range(args.runs):
                start = time.monotonic()
                result = subprocess.run([str(binary.resolve()), '--runtime', 'search'],
                                        input=json.dumps(frame) + '\n', capture_output=True, text=True,
                                        env=environment, cwd=root, check=True, timeout=30)
                samples.append(time.monotonic() - start)
                output = json.loads(json.loads(result.stdout)['data']['output'])
                assert output['files_scanned'] == 8 and output['matches'] == [], output
            print(json.dumps({'binary': str(binary), 'fixture_bytes': len(content) * 8,
                              'seconds': samples, 'median_seconds': statistics.median(samples),
                              'scope': 'includes process startup and filesystem reads; first run may be cold'}))


if __name__ == '__main__':
    main()
