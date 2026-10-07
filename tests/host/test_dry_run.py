#!/usr/bin/env python3
"""Native dry-run acceptance. No VM, network client, BPF, or host process scan."""
# SPDX-License-Identifier: MIT
import codecs
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import signal
import struct
import subprocess
import sys
import termios
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'vm' / 'cases'))
from tui_monitor import Screen


class Terminal:
    def __init__(self, binary, args, state):
        self.master, self.slave = pty.openpty()
        self.original = termios.tcgetattr(self.slave)
        self.screen = Screen()
        self.decoder = codecs.getincrementaldecoder('utf-8')()
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack('HHHH', 32, 120, 0, 0))

        def session():
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)

        self.process = subprocess.Popen(
            # Keep the controlling session alive until terminal flags are checked.
            # macOS invalidates tcgetattr on this PTY once its session exits.
            [sys.executable, '-c',
             'import subprocess,sys,time; rc=subprocess.call(sys.argv[1:]); print("PREVIEW_EXIT="+str(rc),flush=True); time.sleep(1); sys.exit(rc)',
             binary, *args], stdin=self.slave, stdout=self.slave, stderr=self.slave,
            preexec_fn=session,
            env={**os.environ, 'TERM': 'xterm-256color', 'WARDEN_STATE_DIR': str(state)},
        )

    def pump(self):
        if select.select([self.master], [], [], .05)[0]:
            self.screen.feed(self.decoder.decode(os.read(self.master, 65536)))

    def wait(self, predicate):
        deadline = time.monotonic() + 6
        while not predicate():
            self.pump()
            if time.monotonic() > deadline:
                raise AssertionError((self.process.poll(), self.screen.text()))

    def key(self, value):
        os.write(self.master, value)

    def resize(self, cols, rows):
        self.screen.reset(cols, rows)
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack('HHHH', rows, cols, 0, 0))
        self.process.send_signal(signal.SIGWINCH)

    def close(self):
        if self.process.poll() is None:
            self.process.terminate()
            self.process.wait(timeout=5)
        os.close(self.master)
        os.close(self.slave)


def main():
    binary = str(Path(sys.argv[1]).resolve())
    state = Path(binary).parent / f'unused-dry-run-state-{os.getpid()}'
    assert not state.exists()
    for args in [['--lang', 'ja', 'tui', '--dry-run', '-h'], ['tui', '--help', '--lang', 'en']]:
        result = subprocess.run([binary, *args], capture_output=True, text=True, timeout=5)
        assert result.returncode == 0 and '--dry-run' in result.stdout
    for args in [['tui', '--dry-run'], ['tui', '--dry-run', '--enforce']]:
        result = subprocess.run([binary, *args], capture_output=True, timeout=5)
        assert result.returncode != 0
    for args, exit_key in [([], b'q'), (['tui', '--dry-run'], b'\x03')]:
        terminal = Terminal(binary, args, state)
        try:
            text = terminal.screen.text
            terminal.wait(lambda: 'DRY-RUN' in text() and '203.0.113.30:8443' in text())
            terminal.key(b'b')
            terminal.wait(lambda: '確認:' in text())
            terminal.key(b'\x1b')
            terminal.wait(lambda: '取り消しました' in text())
            terminal.key(b'\r')
            terminal.key(b'n')
            terminal.wait(lambda: '試行=4 拒否=0' in text())
            terminal.key(b'b')
            terminal.wait(lambda: '確認:' in text())
            terminal.key(b'\r')
            terminal.wait(lambda: '模擬ルールを追加' in text())
            terminal.key(b'n')
            terminal.wait(lambda: '試行=5 拒否=1' in text() and 'DENY' in text())
            terminal.key(b'\td')
            terminal.wait(lambda: '解除する' in text())
            terminal.key(b'\r')
            terminal.wait(lambda: '模擬ルールを解除' in text())
            terminal.key(b'n')
            terminal.wait(lambda: '試行=6 拒否=1' in text())
            terminal.resize(40, 10)
            terminal.wait(lambda: 'DRY-RUN' in text() and '広げてください' in text())
            terminal.resize(80, 24)
            terminal.wait(lambda: 'DRY-RUN' in text() and '203.0.113.30:8443' in text())
            terminal.key(exit_key)
            terminal.wait(lambda: 'PREVIEW_EXIT=0' in text())
            assert not terminal.screen.alternate
            assert termios.tcgetattr(terminal.slave) == terminal.original
            terminal.process.wait(timeout=5)
            assert terminal.process.returncode == 0
            assert not state.exists(), 'dry-run entered the VM state/setup path'
        finally:
            terminal.close()
    print(json.dumps({'default_and_explicit_preview': True, 'cancel_deny_retry_remove': True,
                      'resize_label_preserved': True, 'q_ctrl_c_terminal_restored': True,
                      'non_tty_and_mixed_enforce_rejected': True, 'no_vm_state_created': True}))


if __name__ == '__main__':
    main()
