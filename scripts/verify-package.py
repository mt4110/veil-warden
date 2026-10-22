#!/usr/bin/env python3
"""Read-only package consistency check, not publisher authentication."""
# SPDX-License-Identifier: MIT
import hashlib
import json
from pathlib import Path
import sys

FILES = ('bin/veil-warden', 'lib/veil-warden/veil-warden-ebpf')
MACHINES = {'x86_64-linux': 62, 'aarch64-linux': 183}


def verify(root):
    root = Path(root)
    path = root / 'share/veil-warden/manifest.json'
    if path.is_symlink() or path.stat().st_size > 16384:
        raise ValueError('Invalid manifest file')
    manifest = json.loads(path.read_text())
    if (not isinstance(manifest, dict)
            or manifest.get('schema') != 'veil-warden-package-v1'
            or manifest.get('system') not in MACHINES
            or manifest.get('event_abi') != 2
            or manifest.get('bpf_target') != 'bpfel-unknown-none'
            or manifest.get('bpf_cpu') != 'v3'
            or manifest.get('loader_scope') != 'dedicated-sandbox-only'
            or manifest.get('deployment_ready') is not False
            or set(manifest.get('sha256', {})) != set(FILES)):
        raise ValueError('Unsupported package manifest')
    for name, machine in zip(FILES, (MACHINES[manifest['system']], 247)):
        binary = root / name
        if binary.is_symlink() or not binary.is_file():
            raise ValueError(f'Expected a regular artifact: {name}')
        digest = hashlib.sha256()
        with binary.open('rb') as handle:
            header = handle.read(20)
            digest.update(header)
            for chunk in iter(lambda: handle.read(1024 * 1024), b''):
                digest.update(chunk)
        if (len(header) < 20 or header[:6] != b'\x7fELF\x02\x01'
                or int.from_bytes(header[18:20], 'little') != machine):
            raise ValueError(f'Wrong ELF architecture: {name}')
        if digest.hexdigest() != manifest['sha256'][name]:
            raise ValueError(f'Artifact digest mismatch: {name}')
    return manifest


if __name__ == '__main__':
    if len(sys.argv) != 2:
        raise SystemExit('Usage: python3 scripts/verify-package.py PACKAGE_DIRECTORY')
    try:
        verify(sys.argv[1])
    except (OSError, ValueError, TypeError, KeyError) as error:
        raise SystemExit(f'Package verification failed: {error}')
    print('Package consistency verified; publisher signature and runtime support are unverified.')
