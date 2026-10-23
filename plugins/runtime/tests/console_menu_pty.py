"""Offline PTY smoke test: cargo build -p i-plugins; python3 this_file.py."""

import fcntl
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import sys
import tempfile
import termios
import time


binary = Path(sys.argv[1] if len(sys.argv) > 1 else "target/debug/i-plugins").resolve()
with tempfile.TemporaryDirectory(prefix="i-slash-pty-") as root:
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 110, 0, 0))
    env = dict(os.environ, HOME=root, TERM="xterm-256color", NO_COLOR="1")
    for key, name in [("I_CONFIG_DIR", "config"), ("I_DATA_DIR", "data"),
                      ("I_CACHE_DIR", "cache"), ("I_RUNTIME_DIR", "runtime")]:
        env[key] = os.path.join(root, name)
    process = subprocess.Popen(
        [str(binary), "console", "--no-logo", "--bin", "/usr/bin/false", "-C", root],
        stdin=slave, stdout=slave, stderr=slave, env=env, cwd=root,
    )
    os.close(slave)

    def read(delay=0.25):
        end = time.monotonic() + delay
        data = b""
        while time.monotonic() < end:
            ready, _, _ = select.select([master], [], [], max(0, end - time.monotonic()))
            if ready:
                try:
                    data += os.read(master, 65536)
                except OSError:
                    break
        return data

    def send(data):
        os.write(master, data)
        return read()

    try:
        read()
        menu = send(b"/")
        assert b"Show available commands" in menu and b"/mode" in menu, menu
        filtered = send(b"mo")
        assert b"/mode" in filtered and b"Show available commands" not in filtered
        filled = send(b"\r")
        assert b"/mode ask" in filled and b"[mode:" not in filled
        chosen = send(b"\x1b[B\t")
        assert b"/mode auto" in chosen and b"[mode:" not in chosen
        assert b"[mode: auto;" in send(b"\r")
        pasted = send(b"\x1b[200~/exit\n/mode ask\x1b[201~")
        assert process.poll() is None and b"exit requested" not in pasted
        assert b"[mode: ask;" not in pasted
        rejected = send(b"\r")
        assert b"invalid slash command arguments" in rejected or b"unknown or multiline slash command" in rejected
        assert b"unknown or multiline slash command" in send(b"/not-real\r")
        send(b"/help")
        send(b"\x1b")
        time.sleep(0.15)
        read()
        assert b"Show available commands" in send(b"\r")
        send(b"/ex\t")
        assert process.poll() is None
        send(b"\r")
        process.wait(timeout=3)
        assert process.returncode == 0
        print("PASS: slash menu, prefix, arrows, fill-only, explicit execution, Escape, unknown command, paste, exit")
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=3)
        os.close(master)
