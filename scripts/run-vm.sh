#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && pwd)
state="${WARDEN_STATE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/veil-warden-m0}"
if [[ "$(uname -m)" != arm64 && "$(uname -m)" != aarch64 ]]; then
  echo "The M0 VM requires an ARM64 host." >&2
  exit 1
fi
umask 077
mkdir -p "$state/sandbox/runs" "$state/private" "$state/public-keys"
if [[ ! -e "$state/private/operator" ]]; then
  ssh-keygen -q -t ed25519 -N '' -C 'veil-warden-local-vm' -f "$state/private/operator"
fi
cp "$state/private/operator.pub" "$state/public-keys/operator.pub"
python3 "$repo/scripts/validate-public-keys.py" "$state/public-keys"
keys=$(nix-store --add "$state/public-keys")
run=$(mktemp -d "$state/sandbox/runs/boot.XXXXXXXX")
printf '%s\n' "$run" > "$state/sandbox/current-run"
bundle=$(cat "$state/sandbox/build-path")
ref=$(python3 - "$repo/flake.lock" <<'PY'
import json, sys
print('github:NixOS/nixpkgs/' + json.load(open(sys.argv[1]))['nodes']['nixpkgs']['locked']['rev'])
PY
)
if [[ "$(uname -s)" == Darwin ]]; then
  qemu=$(nix build "$ref#darwin.linux-builder.nixosConfig.virtualisation.qemu.package" --no-link --print-out-paths)
  accelerator="hvf:tcg"
else
  qemu=$(nix build "$ref#qemu_kvm" --no-link --print-out-paths)
  accelerator="kvm:tcg"
fi
read -r cpus memory port < <(python3 - "$repo/config/sandbox.toml" <<'PY'
import sys, tomllib
m = tomllib.load(open(sys.argv[1], 'rb'))['m0']
if m['ssh_bind'] != '127.0.0.1' or m['scope'] != '/warden.slice/warden-test.slice' or m['attach_enabled']:
    raise SystemExit('M0 policy is outside the allowed sandbox')
print(m['cpus'], m['memory_mib'], m['ssh_port'])
PY
)
exec "$qemu/bin/qemu-system-aarch64" \
  -machine "virt,gic-version=2,accel=$accelerator" -cpu max \
  -name veil-warden-sandbox -m "$memory" -smp "$cpus" \
  -device virtio-rng-pci \
  -netdev "user,id=net0,hostfwd=tcp:127.0.0.1:$port-:22" \
  -device virtio-net-pci,netdev=net0 \
  -drive "file=$bundle/store.img,format=raw,readonly=on,if=none,id=store" \
  -device virtio-blk-pci,drive=store \
  -virtfs "local,path=$keys,security_model=none,mount_tag=keys,readonly=on" \
  -kernel "$bundle/kernel" -initrd "$bundle/initrd" \
  -append "$(cat "$bundle/kernel-params")" -nographic
