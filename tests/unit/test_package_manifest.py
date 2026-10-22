# SPDX-License-Identifier: MIT
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('verify_package', ROOT / 'scripts/verify-package.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class PackageManifest(unittest.TestCase):
    def setUp(self):
        # Retained synthetic fixtures, never real artifacts or an executable.
        self.root = Path(tempfile.mkdtemp(prefix='warden-package-'))
        self.manifest = dict(schema='veil-warden-package-v1', system='x86_64-linux',
                             event_abi=2, bpf_target='bpfel-unknown-none', bpf_cpu='v3',
                             loader_scope='dedicated-sandbox-only', deployment_ready=False,
                             sha256={})
        for name, machine in zip(module.FILES, (62, 247)):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            data = b'\x7fELF\x02\x01' + bytes(12) + machine.to_bytes(2, 'little')
            path.write_bytes(data)
            self.manifest['sha256'][name] = hashlib.sha256(data).hexdigest()
        (self.root / 'share/veil-warden').mkdir(parents=True)
        self.save()

    def save(self):
        (self.root / 'share/veil-warden/manifest.json').write_text(json.dumps(self.manifest))

    def test_matching_pair_passes_consistency_only(self):
        self.assertFalse(module.verify(self.root)['deployment_ready'])

    def test_stale_or_tampered_object_fails(self):
        with (self.root / module.FILES[1]).open('ab') as handle:
            handle.write(b'wrong generation')
        with self.assertRaisesRegex(ValueError, 'digest mismatch'):
            module.verify(self.root)

    def test_wrong_cpu_fails_even_with_matching_digest(self):
        self.manifest['system'] = 'aarch64-linux'
        self.save()
        with self.assertRaisesRegex(ValueError, 'ELF architecture'):
            module.verify(self.root)

    def test_unknown_abi_and_unsafe_claim_fail(self):
        for key, value in [('event_abi', 3), ('deployment_ready', True), ('loader_scope', 'all-host')]:
            old = self.manifest[key]
            self.manifest[key] = value
            self.save()
            with self.assertRaisesRegex(ValueError, 'Unsupported'):
                module.verify(self.root)
            self.manifest[key] = old


if __name__ == '__main__':
    unittest.main()
