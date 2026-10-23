#!/usr/bin/env python3
"""Offline POSIX PTY setup cancellation checks; never touches OS credential stores."""
import os
import json
from pathlib import Path
import pty
import select
import signal
import subprocess
import sys
import tempfile
import termios
import time


def check(binary, terminate):
    with tempfile.TemporaryDirectory(prefix='i-setup-pty-') as directory:
        root = Path(directory)
        environment = os.environ.copy()
        for name in ('CONFIG', 'DATA', 'CACHE', 'RUNTIME'):
            environment[f'I_{name}_DIR'] = str(root / name.lower())
        environment.pop('DEEPSEEK_API_KEY', None)
        master, slave = pty.openpty()
        before = termios.tcgetattr(slave)
        process = subprocess.Popen([binary, 'setup', '--provider', 'deepseek', '--url',
                                    'http://127.0.0.1:9', '--model', 'offline-fixture'],
                                   stdin=slave, stdout=slave, stderr=slave, env=environment)
        output = bytearray()
        def until(marker):
            deadline = time.monotonic() + 10
            while marker not in output:
                assert time.monotonic() < deadline, bytes(output)
                if select.select([master], [], [], 0.1)[0]:
                    output.extend(os.read(master, 65536))
                assert process.poll() is None, bytes(output)
        try:
            until(b'Key storage (env / store)')
            os.write(master, b'store\n')
            until(b'API key (hidden, stored in OS credential storage):')
            assert not termios.tcgetattr(slave)[3] & termios.ECHO
            os.write(master, b'never-echo-this-fake-token')
            time.sleep(0.15)
            if terminate:
                process.send_signal(signal.SIGTERM)
            else:
                os.write(master, b'\x03')
            assert process.wait(timeout=5) != 0
            while select.select([master], [], [], 0.1)[0]:
                output.extend(os.read(master, 65536))
            assert b'never-echo-this-fake-token' not in output, bytes(output)
            after = termios.tcgetattr(slave)
            # Darwin may set the kernel-managed retype-pending bit when returning
            # from raw mode after canonical prompts. Compare all user settings.
            pending = getattr(termios, 'PENDIN', 0)
            after[3] &= ~pending
            before[3] &= ~pending
            assert after == before, (before, after, bytes(output))
            assert not (root / 'config/config.json').exists()
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
            os.close(master)
            os.close(slave)


def check_private_url(binary):
    with tempfile.TemporaryDirectory(prefix='i-setup-url-') as directory:
        root = Path(directory)
        config = root / 'config'
        config.mkdir()
        original = json.dumps({'protocol': 'deepseek', 'base_url':
                               'https://user:never-print-url-secret@localhost', 'model': 'offline'})
        (config / 'config.json').write_text(original)
        environment = os.environ.copy()
        for name in ('CONFIG', 'DATA', 'CACHE', 'RUNTIME'):
            environment[f'I_{name}_DIR'] = str(root / name.lower())
        master, slave = pty.openpty()
        try:
            result = subprocess.run([binary, 'setup', '--provider', 'deepseek', '--model',
                                     'offline', '--key-env', 'I_SETUP_FAKE', '--confirm'],
                                    stdin=slave, capture_output=True, env=environment, timeout=5)
            assert result.returncode != 0
            assert b'never-print-url-secret' not in result.stdout + result.stderr
            assert (config / 'config.json').read_text() == original
        finally:
            os.close(master)
            os.close(slave)


if __name__ == '__main__':
    binary = str(Path(sys.argv[1]).resolve())
    check(binary, False)
    check(binary, True)
    check_private_url(binary)
    print('PASS: hidden setup input, Ctrl-C/SIGTERM restoration, no saved config or OS-store writes')
