#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && pwd)
state="${WARDEN_STATE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/veil-warden-m0}"
if [[ "$(uname -s)" != Darwin || "$(uname -m)" != arm64 ]]; then
  echo "This bootstrap helper requires an Apple Silicon Mac." >&2
  exit 1
fi
umask 077
mkdir -p "$state/bootstrap" "$state/private" "$state/public-keys"
if [[ ! -e "$state/private/operator" ]]; then
  ssh-keygen -q -t ed25519 -N '' -C 'veil-warden-local-vm' -f "$state/private/operator"
fi
cp "$state/private/operator.pub" "$state/public-keys/operator.pub"
cp "$state/private/operator.pub" "$state/public-keys/builder_ed25519.pub"
ref=$(python3 - "$repo/flake.lock" <<'PY'
import json, sys
lock = json.load(open(sys.argv[1]))['nodes']['nixpkgs']['locked']
print('github:NixOS/nixpkgs/' + lock['rev'])
PY
)
# Download the unmodified cached bootstrap closure; skip its sudo installer.
nix build "$ref#darwin.linux-builder.run-builder" --no-link > "$state/bootstrap/build.log" 2>&1
vm=$(nix eval "$ref#darwin.linux-builder.nixosConfig.system.build.vm" --raw)
source=$(nix flake metadata "$ref" --json | python3 -c 'import json,sys; print(json.load(sys.stdin)["path"])')
awk '{ print "[127.0.0.1]:32222 " $1 " " $2 }' \
  "$source/nixos/modules/profiles/keys/ssh_host_ed25519_key.pub" > "$state/bootstrap/known_hosts"
python3 - "$vm/bin/run-nixos-vm" "$state/bootstrap/run-vm" "$repo/config/sandbox.toml" <<'PY'
from pathlib import Path
import sys, tomllib
limits = tomllib.load(open(sys.argv[3], "rb"))["bootstrap"]
if limits != {"cpus": 2, "memory_mib": 4096, "disk_mib": 32768}:
 raise SystemExit("Bootstrap resource policy changed; review before launch")
s = Path(sys.argv[1]).read_text()
replacements = {
 'hostfwd=tcp::31022-:22': 'hostfwd=tcp:127.0.0.1:32222-:22',
 '-m 3072': f'-m {limits["memory_mib"]}',
 '-smp 1': f'-smp {limits["cpus"]}',
 '"20480M"': f'"{limits["disk_mib"]}M"',
 'mkdir -p "$TMPDIR/certs"': 'mkdir -p "$TMPDIR/certs"\nchmod 0755 "$TMPDIR/certs"',
 '  cp -L "$NIX_SSL_CERT_FILE" "$TMPDIR"/certs/ca-certificates.crt': '  cp -L "$NIX_SSL_CERT_FILE" "$TMPDIR"/certs/ca-certificates.crt\n  chmod 0644 "$TMPDIR"/certs/ca-certificates.crt',
 '  rm "$temp"': '  : # Retain the newly created raw image; no deletion is performed.',
}
for old, new in replacements.items():
 if s.count(old) != 1:
  raise SystemExit('Pinned bootstrap layout changed: review launcher before use')
 s = s.replace(old, new)
p = Path(sys.argv[2])
p.write_text(s)
p.chmod(0o700)
PY
python3 "$repo/scripts/validate-public-keys.py" "$state/public-keys"
KEYS="$(nix-store --add "$state/public-keys")"
export KEYS
TMPDIR="$state/bootstrap/runtime-$(date +%Y%m%dT%H%M%S)"
export TMPDIR
mkdir -p "$TMPDIR"
export USE_TMPDIR=1
export NIX_DISK_IMAGE="$state/bootstrap/root-32g.qcow2"
export QEMU_NET_OPTS=""
export SHARED_DIR="$TMPDIR/xchg"
NIX_SSL_CERT_FILE="$(nix build "$ref#cacert" --no-link --print-out-paths)/etc/ssl/certs/ca-bundle.crt"
export NIX_SSL_CERT_FILE
exec "$state/bootstrap/run-vm"
