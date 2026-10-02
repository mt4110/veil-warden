#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
set -euo pipefail
cd "$(dirname "$0")/.."
nix --extra-experimental-features "nix-command flakes" develop "path:$PWD#bpf" --command cargo build --locked -p veil-warden-ebpf --features ebpf \
  --target bpfel-unknown-none -Z build-std=core --release
nix --extra-experimental-features "nix-command flakes" develop "path:$PWD" --command cargo build --locked -p veil-warden --release
