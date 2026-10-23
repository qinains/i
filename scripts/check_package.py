#!/usr/bin/env python3
"""Offline integration check for a generated package; no real credentials or paid API."""
import hashlib
import glob
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import threading


def check(archive):
    with tempfile.TemporaryDirectory(prefix='i-package-check-') as temporary:
        root = Path(temporary)
        with tarfile.open(archive) as package:
            package.extractall(root, filter='data')
        bundle = next(root.iterdir())
        for line in (bundle / 'SHA256SUMS').read_text().splitlines():
            digest, name = line.split('  ', 1)
            assert hashlib.sha256((bundle / name).read_bytes()).hexdigest() == digest, name
        suffix = '.exe' if os.name == 'nt' else ''
        core = bundle / 'bin' / ('i' + suffix)
        helper = [core, '--runtime']
        metadata = json.loads((bundle / 'BUILD.json').read_text())
        assert metadata['layout'] == 'bundled'
        assert metadata['binary_total_bytes'] == core.stat().st_size
        sidecar = Path(str(archive) + '.sha256')
        assert sidecar.read_text().split()[0] == hashlib.sha256(archive.read_bytes()).hexdigest()
        assert len(list((bundle / 'bin').iterdir())) == 1
        if os.name != 'nt':
            subprocess.run([sys.executable, str(Path(__file__).with_name('check_tls.py')), str(core), '--web'],
                           check=True, timeout=45)
        environment = os.environ.copy()
        environment['I_PACKAGE_TEST_KEY'] = 'offline-fixture-only'
        environment['NO_PROXY'] = environment['no_proxy'] = '127.0.0.1,localhost'
        for name in ('CONFIG', 'DATA', 'CACHE', 'RUNTIME'):
            environment[f'I_{name}_DIR'] = str(root / name.lower())
        def run(argv):
            return subprocess.run([str(value) for value in argv], env=environment, cwd=root,
                                  stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=45, check=True)
        assert 'i 0.1.0' in run([core, '--version']).stdout
        assert 'cx="171.5"' in run([*helper, 'logo', '--svg']).stdout
        run([*helper, 'init'])
        installed = root / 'installed/bin'
        run([core, 'install', '--bin-dir', installed])
        bundle.rename(root / 'source-moved-away')
        core = installed / ('i' + suffix)
        helper = [core, '--runtime']
        manifest = json.loads(run([*helper, 'manifest']).stdout)
        for entry in manifest['tools'] + manifest['hooks'] + [manifest['provider']]:
            assert entry['exec'][0] == str(core) and entry['exec'][1] == '--runtime'
        workspace = root / 'project'
        workspace.mkdir()
        (workspace / 'unicode.txt').write_text('Hello 你好 Σ K\n', encoding='utf-8')
        for query, regex in [('hello', False), ('σ', False), (r'\p{Script=Han}+', True)]:
            frame = {'id': 1, 'event': 'tool', 'workspace': str(workspace),
                     'data': {'name': 'search', 'arguments': {'query': query, 'regex': regex}}}
            worker = subprocess.run([str(value) for value in [*helper, 'search']],
                                    input=json.dumps(frame) + '\n', env=environment, cwd=root,
                                    capture_output=True, text=True, timeout=20, check=True)
            reply = json.loads(worker.stdout)
            matches = json.loads(reply['data']['output'])['matches']
            assert len(matches) == 1 and matches[0]['path'] == 'unicode.txt', reply
        requests = []
        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass
            def do_POST(self):
                request = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
                requests.append(request)
                message = {'role':'assistant', 'content':'packaged-ok'}
                finish = 'stop'
                if len(requests) == 1:
                    message.update(content=None, tool_calls=[{'id':'package-call', 'type':'function',
                        'function':{'name':'memory_set','arguments':json.dumps({'key':'package_check','value':'ok'})}}])
                    finish = 'tool_calls'
                response = json.dumps({'choices':[{'message':message,'finish_reason':finish}],
                    'usage':{'prompt_tokens':256,'prompt_tokens_details':{'cached_tokens':0},'completion_tokens':4}}).encode()
                self.send_response(200)
                self.send_header('Content-Type','application/json')
                self.send_header('Content-Length',str(len(response)))
                self.end_headers()
                self.wfile.write(response)
        server = ThreadingHTTPServer(('127.0.0.1',0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            config = root / 'config/config.json'
            config.write_text(json.dumps({'protocol':'openai','base_url':f'http://127.0.0.1:{server.server_port}',
                'model':'package-fixture','api_key_env':'I_PACKAGE_TEST_KEY','max_tokens':64,'use_max_tokens':True,
                'plugins':'builtin.json','budget':{'context_window':16384}}))
            console = run([*helper,'console','--no-logo',f'--dir={workspace}','--session-name=check'])
            assert 'Type a task' in console.stderr
            result = run([*helper,'console','--no-logo','--config',config,'-C',workspace,'--max-steps','2','--dangerously-skip-permissions',
                          '--task=Remember the package_check preference as ok.'])
            assert 'packaged-ok' in result.stdout
            assert len(requests) == 2 and len(requests[0]['tools']) == 15
            assert any(message.get('role')=='tool' for message in requests[1]['messages'])
            memory = next((root/'data/workspaces').glob('*/memory.json'))
            assert json.loads(memory.read_text())['package_check'] == 'ok'
            report = json.loads(run([*helper,'cache-report','-C',workspace]).stdout)
            assert report['totals']['observed_responses'] == 2
            print(f'{archive.name}: PASS — hashes, self-install/source relocation, portable bindings, single executable, console task, 15 tools, bundled memory/steering workers, Unicode search, usage-based budget, cache accounting')
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)


if __name__ == '__main__':
    if len(sys.argv) < 2:
        raise SystemExit('Usage: python3 scripts/check_package.py dist/PACKAGE.tar.{xz,gz} [...]')
    for argument in sys.argv[1:]:
        matches = glob.glob(argument)
        if not matches:
            raise SystemExit(f'No archives match: {argument}')
        for match in sorted(matches):
            check(Path(match).resolve())
