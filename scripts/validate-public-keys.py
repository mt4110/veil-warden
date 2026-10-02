#!/usr/bin/env python3
"""Reject non-public material before importing the shared directory to Nix."""
# SPDX-License-Identifier: MIT
import base64
from pathlib import Path
import sys


def valid_public_key(text):
    fields = text.strip().split()
    if len(fields) < 2 or fields[0] != "ssh-ed25519" or "\n" in text.strip():
        return False
    try:
        data = base64.b64decode(fields[1], validate=True)
    except ValueError:
        return False
    return len(data) == 51 and data.startswith(b"\0\0\0\x0bssh-ed25519\0\0\0\x20")


def main():
    directory = Path(sys.argv[1])
    for path in directory.iterdir():
        if path.name not in {"operator.pub", "builder_ed25519.pub"}:
            raise SystemExit("Public-key directory contains an unexpected file")
        if path.is_symlink() or not path.is_file() or path.stat().st_size > 8192:
            raise SystemExit("Invalid public-key file")
        if not valid_public_key(path.read_text(encoding="ascii")):
            raise SystemExit("Only valid Ed25519 public keys may enter the Nix store")
    if not (directory / "operator.pub").is_file():
        raise SystemExit("Operator public key is required")


if __name__ == "__main__":
    main()
