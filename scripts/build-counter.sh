#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
set -euo pipefail
cd "$(dirname "$0")/.."
# Keep compiler output outside the source tree: each path: flake evaluation can
# otherwise copy the previous target into the Nix store before filtering it.
CARGO_TARGET_DIR=$(realpath -m "${CARGO_TARGET_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/veil-warden-build/target}")
case "$CARGO_TARGET_DIR" in
  "$PWD"|"$PWD"/*) echo 'CARGO_TARGET_DIR must be outside the source checkout.' >&2; exit 1 ;;
esac
export CARGO_TARGET_DIR
mkdir -p "$CARGO_TARGET_DIR"
# Hold the lock through the final artifact snapshot, not just one cargo command.
exec 9>"$CARGO_TARGET_DIR/veil-warden-build.lock"
flock -n 9 || { echo 'Another veil-warden build is active; retry after it finishes.' >&2; exit 1; }
nix --extra-experimental-features "nix-command flakes" develop "path:$PWD#bpf" --command cargo build --locked -p veil-warden-ebpf --features ebpf \
  --target bpfel-unknown-none -Z build-std=core --release
nix --extra-experimental-features "nix-command flakes" develop "path:$PWD#bpf" --command cargo build --locked -p veil-warden-ebpf --features partial-fixture \
  --bin veil-warden-partial-fixture --target bpfel-unknown-none -Z build-std=core --release
nix --extra-experimental-features "nix-command flakes" develop "path:$PWD" --command cargo build --locked -p veil-warden --release
output="${WARDEN_BUILD_OUTPUT:-$PWD/artifacts/build}"
mkdir -p "$output"
cp "$CARGO_TARGET_DIR/release/veil-warden" "$output/"
cp "$CARGO_TARGET_DIR/bpfel-unknown-none/release/veil-warden-ebpf" "$output/"
cp "$CARGO_TARGET_DIR/bpfel-unknown-none/release/veil-warden-partial-fixture" "$output/"
printf 'Build artifacts: %s\nShared Cargo cache: %s\n' "$output" "$CARGO_TARGET_DIR"
