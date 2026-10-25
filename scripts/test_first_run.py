"""Offline PTY onboarding regression: I_TEST_BINARY=/absolute/path/to/i python3 -m unittest discover -s scripts -p test_first_run.py."""
import os
from pathlib import Path
import select
import signal
import subprocess
import tempfile
import time
import unittest


@unittest.skipUnless(os.name == "posix" and os.environ.get("I_TEST_BINARY"), "requires POSIX and I_TEST_BINARY")
class FirstRunTests(unittest.TestCase):
    def test_noninteractive_start_does_not_offer_or_install(self):
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            env = os.environ.copy()
            for name in list(env):
                if name.startswith(("I_", "API_")):
                    env.pop(name)
            env["HOME"] = str(home)
            for name in ("CONFIG", "DATA", "CACHE", "RUNTIME"):
                env[f"I_{name}_DIR"] = str(home / name.lower())
            result = subprocess.run([os.environ["I_TEST_BINARY"], "start"], input=b"",
                                    capture_output=True, env=env, timeout=10)
            self.assertNotEqual(result.returncode, 0)
            self.assertNotIn(b"Install for this user?", result.stdout + result.stderr)
            self.assertFalse((home / "config/first-run-install.json").exists())
            self.assertFalse((home / ".local/bin/i").exists())

    def launch(self, home, answer=None):
        import pty

        env = os.environ.copy()
        for name in list(env):
            if name.startswith(("I_", "API_")):
                env.pop(name)
        env.update(HOME=str(home), SHELL="/bin/zsh", PATH="/usr/bin:/bin")
        for name in ("CONFIG", "DATA", "CACHE", "RUNTIME"):
            env[f"I_{name}_DIR"] = str(home / name.lower())
        master, slave = pty.openpty()
        proc = subprocess.Popen([os.environ["I_TEST_BINARY"]], stdin=slave, stdout=slave,
                                stderr=slave, env=env, start_new_session=True)
        os.close(slave)
        output = b""
        answered = False
        try:
            deadline = time.monotonic() + 20
            while time.monotonic() < deadline:
                if select.select([master], [], [], 0.1)[0]:
                    try:
                        output += os.read(master, 65536)
                    except OSError:
                        break
                if answer is not None and not answered and b"Install for this user?" in output:
                    os.write(master, answer)
                    answered = True
                if b"Provider" in output:
                    return output.decode(errors="replace")
            self.fail(f"did not reach setup: {output.decode(errors='replace')}")
        finally:
            if proc.poll() is None:
                os.killpg(proc.pid, signal.SIGKILL)
            proc.wait(timeout=5)
            os.close(master)

    def test_decline_is_once_and_non_destructive(self):
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            self.assertIn("Install for this user?", self.launch(home, b"n\n"))
            self.assertNotIn("Install for this user?", self.launch(home))
            self.assertFalse((home / ".local/bin/i").exists())
            self.assertFalse((home / ".zprofile").exists())
            self.assertFalse((home / "config/config.json").exists())

    def test_accept_installs_and_continues_without_restart(self):
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            self.assertIn("Installed:", self.launch(home, b"y\n"))
            self.assertEqual((home / ".local/bin/i").read_bytes(), Path(os.environ["I_TEST_BINARY"]).read_bytes())
            profile = (home / ".zprofile").read_text()
            self.assertIn(str(home / ".local/bin"), profile)
            self.assertNotIn("Install for this user?", self.launch(home))
            self.assertEqual((home / ".zprofile").read_text(), profile)
