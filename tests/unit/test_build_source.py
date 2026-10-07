# SPDX-License-Identifier: MIT
import importlib.util
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('source_key', ROOT / 'scripts/build-source-key.py')
keys = importlib.util.module_from_spec(spec)
spec.loader.exec_module(keys)


class BuildGeneration(unittest.TestCase):
    def setUp(self):
        # Keep fixtures as evidence; no existing source/cache is deleted.
        self.fixture = Path(tempfile.mkdtemp(prefix='warden-build-'))
        self.repo = self.fixture / 'repo'
        self.repo.mkdir()
        for name in keys.INPUTS:
            path = self.repo / name
            if '.' in name and name != '.cargo':
                path.write_text('fixture')
            else:
                path.mkdir()
        self.source = self.repo / 'crates/main.rs'
        self.source.write_text('generation A')
        for name in ['build-counter.sh', 'build-source-key.py']:
            shutil.copy2(ROOT / 'scripts' / name, self.repo / 'scripts' / name)

    def test_content_key_ignores_mtime_but_tracks_edits_and_removal(self):
        first = keys.source_key(self.repo)
        os.utime(self.source, (1, 1))
        self.assertEqual(first, keys.source_key(self.repo))
        self.source.write_text('generation B')
        os.utime(self.source, (1, 1))
        self.assertNotEqual(first, keys.source_key(self.repo))
        self.source.rename(self.repo / 'removed-source-evidence')
        self.assertNotEqual(first, keys.source_key(self.repo))

    def test_symlink_inputs_fail_closed(self):
        (self.repo / 'crates/link').symlink_to(self.source)
        with self.assertRaises(ValueError):
            keys.source_key(self.repo)

    def test_build_snapshots_do_not_reuse_previous_generation(self):
        shims = self.fixture / 'bin'
        shims.mkdir()
        # Simulate Cargo reusing existing artifacts regardless of source mtimes.
        # A real native compiler regression is run separately in the builder VM.
        nix = shims / 'nix'
        nix.write_text('''#!/usr/bin/env python3
import os, pathlib, subprocess, sys
args = sys.argv[sys.argv.index('--command') + 1:]
if args[0] == 'python3':
    sys.exit(subprocess.call([sys.executable, *args[1:]]))
if args[0] == 'rustc':
    print('host: aarch64-unknown-linux-gnu')
    sys.exit(0)
target = args[args.index('--target') + 1]
name = args[args.index('--bin') + 1] if '--bin' in args else args[args.index('-p') + 1]
dest = pathlib.Path(os.environ['CARGO_TARGET_DIR']) / target / 'release' / name
dest.parent.mkdir(parents=True, exist_ok=True)
if not dest.exists(): dest.write_text(pathlib.Path('crates/main.rs').read_text())
''')
        nix.chmod(0o755)
        # Portable shims allow this orchestration check on macOS as well as Linux.
        for name, text in {
            'realpath': '#!/usr/bin/env python3\nimport os,sys\nprint(os.path.realpath(sys.argv[-1]))\n',
            'flock': '#!/bin/sh\nexit 0\n',
        }.items():
            path = shims / name
            path.write_text(text)
            path.chmod(0o755)
        env = dict(os.environ, PATH=str(shims) + os.pathsep + os.environ['PATH'],
                   CARGO_TARGET_DIR=str(self.fixture / 'cache'),
                   CARGO_BUILD_TARGET='x86_64-unknown-linux-gnu')
        def build():
            subprocess.run(['bash', 'scripts/build-counter.sh'], cwd=self.repo,
                           env=env, check=True, capture_output=True, text=True)
            return (self.repo / 'artifacts/build/cargo-target-dir').read_text()
        first = build()
        self.source.write_text('generation B')
        os.utime(self.source, (1, 1))
        second = build()
        self.assertNotEqual(first, second)
        for name in ['veil-warden', 'veil-warden-ebpf', 'veil-warden-partial-fixture']:
            self.assertEqual('generation B', (self.repo / 'artifacts/build' / name).read_text())
        self.assertEqual(second, build())


if __name__ == '__main__':
    unittest.main()
