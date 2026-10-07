#!/usr/bin/env python3
"""Content identity for local build inputs, independent of Git and timestamps."""
# SPDX-License-Identifier: MIT
import hashlib
from pathlib import Path
import sys

INPUTS = ('Cargo.toml', 'Cargo.lock', 'flake.nix', 'flake.lock',
          'rust-toolchain.toml', '.cargo', 'crates', 'tools', 'config',
          'infra', 'tests', 'scripts')


def source_key(root):
    digest = hashlib.sha256()
    for name in INPUTS:
        entry = root / name
        paths = [entry, *sorted(entry.rglob('*'))] if entry.is_dir() else [entry]
        for path in paths:
            if '__pycache__' in path.parts:
                continue
            if path.is_symlink():
                raise ValueError(f'Symlink in build inputs is unsupported: {path}')
            if path.is_dir():
                continue
            data = path.read_bytes()
            relative = path.relative_to(root).as_posix().encode()
            digest.update(len(relative).to_bytes(8, 'big'))
            digest.update(relative)
            digest.update(len(data).to_bytes(8, 'big'))
            digest.update(data)
    return digest.hexdigest()


if __name__ == '__main__':
    print(source_key(Path(sys.argv[1]).resolve()))
