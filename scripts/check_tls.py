#!/usr/bin/env python3
"""Offline TLS regression: require ClientHello and a certificate error, never just EOF/nonzero exit."""
import argparse
import json
import os
from pathlib import Path
import socket
import ssl
import subprocess
import tempfile
import threading


def check(binary, root, tls, environment, web=False):
    server = socket.socket()
    server.bind(('127.0.0.1', 0))
    server.listen()
    server.settimeout(10)
    port = server.getsockname()[1]
    observed = {'client_hello': False, 'http_received': False}

    def serve():
        try:
            stream, _ = server.accept()
            with stream:
                stream.settimeout(5)
                observed['client_hello'] = stream.recv(1, socket.MSG_PEEK) == b'\x16'
                if observed['client_hello']:
                    with tls.wrap_socket(stream, server_side=True) as secure:
                        # Some providers finish the handshake before reporting trust failure.
                        observed['http_received'] = bool(secure.recv(1))
        except (ssl.SSLError, OSError) as error:
            observed['server_error'] = str(error)
        finally:
            server.close()

    worker = threading.Thread(target=serve, daemon=True)
    worker.start()
    endpoint = f'https://127.0.0.1:{port}'
    if web:
        config = root / 'web.json'
        config.write_text(json.dumps({'allow_private_network': True}))
        command = [str(binary), '--runtime', 'web', '--config', str(config)]
        request = json.dumps({'id': 1, 'event': 'tool', 'workspace': str(root),
                              'data': {'name': 'web_fetch', 'arguments': {'url': endpoint}}}) + '\n'
    else:
        config = root / 'api.json'
        config.write_text(json.dumps({'base_url': endpoint, 'model': 'offline-fixture',
                                      'api_key_env': 'I_TLS_NO_KEY', 'proxy': '', 'network_retries': 5}))
        command = [str(binary), '--config', str(config), '-C', str(root), 'offline fixture']
        request = ''
    try:
        result = subprocess.run(command, input=request, env=environment, capture_output=True,
                                text=True, encoding='utf-8', timeout=10)
    finally:
        worker.join(timeout=12)
    assert not worker.is_alive(), 'TLS fixture did not finish'
    assert 'panicked' not in result.stderr and 'feature is not enabled' not in result.stderr, result.stderr
    assert 'network retry' not in result.stderr, result.stderr
    if web:
        assert result.returncode == 0, result.stderr
        reply = json.loads(result.stdout)
        assert reply.get('id') == 1 and isinstance(reply.get('error'), str), reply
        diagnostic = reply['error']
    else:
        assert result.returncode == 1, f'expected normal certificate error, exit={result.returncode}: {result.stderr}'
        diagnostic = result.stderr
    assert observed['client_hello'], f'no TLS ClientHello (EOF/panic is NOT certificate rejection): {observed}'
    assert not observed['http_received'], f'HTTP sent over an untrusted TLS connection: {observed}'
    assert any(word in diagnostic.lower() for word in ('certificate', 'cert_', 'trust', 'issuer')), diagnostic
    print(f'PASS: {"web worker" if web else "model transport"} sends ClientHello, rejects untrusted certificate, no panic/retry')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', nargs='?', type=Path, default=Path('target/debug/i'))
    parser.add_argument('--web', action='store_true', help='also check the bundled web worker')
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='i-tls-check-') as temporary:
        root = Path(temporary)
        config = root / 'certificate.cnf'
        config.write_text('[req]\nprompt=no\ndistinguished_name=dn\nx509_extensions=ext\n'
                          '[dn]\nCN=localhost\n[ext]\nsubjectAltName=IP:127.0.0.1,DNS:localhost\n'
                          'basicConstraints=critical,CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\n'
                          'extendedKeyUsage=serverAuth\n')
        subprocess.run(['openssl', 'req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-sha256', '-days', '1',
                        '-config', str(config), '-keyout', str(root / 'key.pem'), '-out', str(root / 'cert.pem')],
                       check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        tls.load_cert_chain(root / 'cert.pem', root / 'key.pem')
        environment = os.environ.copy()
        for name in ('API_KEY', 'DEEPSEEK_API_KEY', 'DEEPSEEK_TEST_API_KEY', 'OPENAI_API_KEY', 'I_TLS_NO_KEY'):
            environment.pop(name, None)
        for name in ('CONFIG', 'DATA', 'CACHE', 'RUNTIME'):
            environment[f'I_{name}_DIR'] = str(root / name.lower())
        check(args.binary.resolve(), root, tls, environment)
        if args.web:
            check(args.binary.resolve(), root, tls, environment, web=True)


if __name__ == '__main__':
    main()
