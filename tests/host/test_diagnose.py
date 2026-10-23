#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Exercise the real CLI's non-loading entry point on the local host."""
import platform
from pathlib import Path
import subprocess
import sys
import unittest

BINARY = str(Path(sys.argv.pop(1)).resolve())


class DiagnoseCLI(unittest.TestCase):
    def run_cli(self, *args):
        return subprocess.run([BINARY, *args], capture_output=True, text=True, timeout=5)

    def test_help_without_loading(self):
        for option in ('--help', '-h'):
            result = self.run_cli('diagnose', option)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn('BPF', result.stdout)
            self.assertIn('--lang', result.stdout)

    def test_languages_and_non_ready_contract(self):
        for language, phrase in [('ja', '現行loader'), ('en', 'Current loader')]:
            result = self.run_cli('diagnose', '--cgroup', '/system.slice/example.service', '--lang', language)
            self.assertEqual(result.returncode, 1, result.stderr)
            for field in ('read_only=true', 'bpf_loaded=false', 'apply_supported=false',
                          'deployment_ready=false', 'loader_scope=blocked', 'runtime_acceptance=unknown'):
                self.assertIn(field, result.stdout)
            self.assertIn(phrase, result.stdout)
            if platform.system() != 'Linux':
                self.assertIn('platform=blocked', result.stdout)
            self.assertNotIn('attached mode=', result.stdout)

    def test_service_mode_without_runtime_loading(self):
        result = self.run_cli('diagnose', '--service', 'arena-fixture.service', '--lang', 'en')
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn('service_identity=unknown', result.stdout)
        self.assertIn('bpf_loaded=false', result.stdout)
        self.assertIn('deployment_ready=false', result.stdout)
        for unit in ('ssh.service', 'docker.service', '../app.service', 'app@.service'):
            bad = self.run_cli('diagnose', '--service', unit)
            self.assertEqual(bad.returncode, 2)
            self.assertEqual(bad.stdout, '')
        mixed = self.run_cli('diagnose', '--service', 'app.service', '--cgroup', '/a/app.service')
        self.assertEqual(mixed.returncode, 2)

    def test_invalid_requests_do_not_reach_diagnosis_or_loader(self):
        for args in [[], ['--cgroup', '/'], ['--cgroup', '/system.slice/ssh.service'],
                     ['--cgroup', '/a/x.service', '--enforce'], ['--help', '--enforce'],
                     ['--cgroup', '/a/x.service', '--lang', 'fr']]:
            result = self.run_cli('diagnose', *args)
            self.assertEqual(result.returncode, 2, result.stderr)
            self.assertEqual(result.stdout, '')


if __name__ == '__main__':
    unittest.main()
