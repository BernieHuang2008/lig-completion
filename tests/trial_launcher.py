#!/usr/bin/env python3
"""Test the release trial launcher, command popups, and retained diagnostics."""
import os
import pty
import re
import shutil
import signal
import sys
import time
import unittest
from pathlib import Path
from fish_pty import BIN, FISH, Fish, ROOT

@unittest.skipUnless(FISH, 'fish is unavailable')
class TrialLauncher(unittest.TestCase):
    def test_debug_trial(self):
        if not BIN.exists():
            self.skipTest('cargo build --release is required')
        pid, fd = pty.fork()
        if pid == 0:
            os.environ['TERM'] = 'xterm-256color'
            os.execv(sys.executable, [sys.executable, str(ROOT / 'tools/try-fish.py'), '--debug', '--bin', str(BIN)])
        term = object.__new__(Fish)
        term.pid, term.fd = pid, fd
        debug = None
        reaped = False
        try:
            term.output = b''
            term.wait_for(lambda: b'lig-test ' in term.output)
            startup = term.output
            match = re.search(rb'Debug log: ([^\r\n]+)', startup)
            self.assertIsNotNone(match, startup)
            debug = Path(os.fsdecode(match[1]))
            home = Path(os.fsdecode(re.search(rb'updated (.*)/config/fish/config.fish', startup)[1]))
            start = len(term.output)
            term.send('car')
            output = term.wait_text('cargo', start)
            plain = re.sub(rb'\x1b\[[0-?]*[ -/]*[@-~]', b'', output)
            self.assertIn(b'cargo', plain)
            term.send(b'\x03')
            start = len(term.output)
            term.send('git ch')
            output = term.wait_text('checkout', start)
            plain = re.sub(rb'\x1b\[[0-?]*[ -/]*[@-~]', b'', output)
            self.assertIn(b'checkout', plain)
            term.send(b'\x03')
            term.send('cargo --version > "$HOME/version"\r')
            term.wait_for(lambda: (home / 'version').exists() and (home / 'version').stat().st_size > 0)
            self.assertTrue((home / 'version').read_text().startswith('cargo '))
            term.send(b'\x03')
            term.send('exit\r')
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                term.read(.05)
                found, status = os.waitpid(pid, os.WNOHANG)
                if found:
                    reaped = True
                    self.assertEqual(os.waitstatus_to_exitcode(status), 0)
                    break
            self.assertTrue(reaped, 'launcher did not exit')
            self.assertFalse(home.exists())
            log = debug.read_text()
            self.assertIn('reader ready fish=true', log)
            self.assertIn('rect=Some', log)
            self.assertNotIn('git ch', log)
        finally:
            if not reaped:
                os.kill(pid, signal.SIGTERM)
                os.waitpid(pid, 0)
            os.close(fd)
            if debug is not None:
                shutil.rmtree(debug.parent.parent)

if __name__ == '__main__':
    unittest.main(verbosity=2)
