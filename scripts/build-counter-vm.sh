#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && pwd)
state="${WARDEN_STATE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/veil-warden-m0}"
if [[ "$(uname -s)" != Darwin || "$(uname -m)" != arm64 ]]; then
  echo "Use build-counter.sh directly on Linux; this helper targets Apple Silicon." >&2
  exit 1
fi
umask 077
revision=$(date +%Y%m%dT%H%M%S)-$$
remote="/home/builder/warden-m1-$revision"
destination="$state/m1/build-$revision"
mkdir -p "$destination"
options=(-i "$state/private/operator" -o BatchMode=yes -o IdentitiesOnly=yes
  -o StrictHostKeyChecking=yes -o "UserKnownHostsFile=$state/bootstrap/known_hosts")
COPYFILE_DISABLE=1 tar --no-xattrs -czf "$destination/source.tar.gz" -C "$repo" \
  flake.nix flake.lock Cargo.toml Cargo.lock rust-toolchain.toml .cargo crates \
  config infra tests tools scripts/build-counter.sh
scp "${options[@]}" -P 32222 "$destination/source.tar.gz" "builder@127.0.0.1:m1-source-$revision.tar.gz"
ssh "${options[@]}" -p 32222 builder@127.0.0.1 \
  "set -eu; mkdir -p '$remote'; tar -xzf 'm1-source-$revision.tar.gz' -C '$remote'; cd '$remote'; ./scripts/build-counter.sh"
scp "${options[@]}" -P 32222 "builder@127.0.0.1:$remote/target/release/veil-warden" "$destination/"
scp "${options[@]}" -P 32222 "builder@127.0.0.1:$remote/target/bpfel-unknown-none/release/veil-warden-ebpf" "$destination/"
scp "${options[@]}" -P 32222 "builder@127.0.0.1:$remote/target/bpfel-unknown-none/release/veil-warden-partial-fixture" "$destination/"
printf '%s\n' "$destination" > "$state/m1/build-path"
printf 'M1 artifacts: %s\n' "$destination"
