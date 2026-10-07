#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
set -euo pipefail
cd "$(dirname "$0")/.."
# Keep compiler output outside the source tree: each path: flake evaluation can
# otherwise copy the previous target into the Nix store before filtering it.
cache_root=$(realpath -m "${CARGO_TARGET_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/veil-warden-build/target}")
case "$cache_root" in
  "$PWD"|"$PWD"/*) echo 'CARGO_TARGET_DIR must be outside the source checkout.' >&2; exit 1 ;;
esac
mkdir -p "$cache_root"
# Hold the lock through the final artifact snapshot, not just one cargo command.
exec 9>"$cache_root/veil-warden-build.lock"
flock -n 9 || { echo 'Another veil-warden build is active; retry after it finishes.' >&2; exit 1; }
source_identity() {
  nix --extra-experimental-features "nix-command flakes" develop "path:$PWD" \
    --command python3 scripts/build-source-key.py "$PWD" | tail -n 1
}
source_key=$(source_identity)
[[ "$source_key" =~ ^[0-9a-f]{64}$ ]] || { echo 'Invalid build source identity.' >&2; exit 1; }
# Restored sources can have old mtimes. Never reuse another content generation.
export CARGO_TARGET_DIR="$cache_root/generations/$source_key"
mkdir -p "$CARGO_TARGET_DIR"
host_target=$(nix --extra-experimental-features "nix-command flakes" develop "path:$PWD" --command rustc -vV | sed -n 's/^host: //p')
case "$host_target" in
  *-linux-*) ;;
  *) echo 'build-counter.sh requires a native Linux toolchain.' >&2; exit 1 ;;
esac
nix --extra-experimental-features "nix-command flakes" develop "path:$PWD#bpf" --command cargo build --locked -p veil-warden-ebpf --features ebpf \
  --target bpfel-unknown-none -Z build-std=core --release
nix --extra-experimental-features "nix-command flakes" develop "path:$PWD#bpf" --command cargo build --locked -p veil-warden-ebpf --features partial-fixture \
  --bin veil-warden-partial-fixture --target bpfel-unknown-none -Z build-std=core --release
nix --extra-experimental-features "nix-command flakes" develop "path:$PWD" --command cargo build --locked -p veil-warden \
  --bin veil-warden --target "$host_target" --release
if [[ "$(source_identity)" != "$source_key" ]]; then
  echo 'Build inputs changed during compilation; refusing to publish artifacts.' >&2
  exit 1
fi
output="${WARDEN_BUILD_OUTPUT:-$PWD/artifacts/build}"
mkdir -p "$output"
cp "$CARGO_TARGET_DIR/$host_target/release/veil-warden" "$output/"
cp "$CARGO_TARGET_DIR/bpfel-unknown-none/release/veil-warden-ebpf" "$output/"
cp "$CARGO_TARGET_DIR/bpfel-unknown-none/release/veil-warden-partial-fixture" "$output/"
printf '%s\n' "$CARGO_TARGET_DIR" > "$output/cargo-target-dir"
printf '%s\n' "$source_key" > "$output/build-source-key"
printf 'Build artifacts: %s\nGeneration Cargo cache: %s\n' "$output" "$CARGO_TARGET_DIR"
