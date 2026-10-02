# SPDX-License-Identifier: MIT
import base64
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location(
    "keys", Path(__file__).resolve().parents[2] / "scripts/validate-public-keys.py")
keys = importlib.util.module_from_spec(spec)
spec.loader.exec_module(keys)


class PublicKeyBoundary(unittest.TestCase):
    def test_public_wire_format(self):
        raw = b"\0\0\0\x0bssh-ed25519\0\0\0\x20" + bytes(32)
        self.assertTrue(keys.valid_public_key("ssh-ed25519 " + base64.b64encode(raw).decode()))

    def test_private_and_malformed_material_is_rejected(self):
        for text in ["-----BEGIN OPENSSH PRIVATE KEY-----", "ssh-ed25519 invalid!",
                     "ssh-ed25519 AAAA", "ssh-rsa AAAA", "ssh-ed25519 AAAA\nsecret"]:
            self.assertFalse(keys.valid_public_key(text))


if __name__ == "__main__":
    unittest.main()
