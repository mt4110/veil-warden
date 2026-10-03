#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && pwd)
state="${WARDEN_STATE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/veil-warden-m0}"
umask 077
mkdir -p "$state/sandbox"
if [[ "$(uname -s)" == Darwin ]]; then
  ssh_options=(-F /dev/null -o ConnectTimeout=5 -i "$state/private/operator" -o BatchMode=yes -o IdentitiesOnly=yes
    -o StrictHostKeyChecking=yes -o "UserKnownHostsFile=$state/bootstrap/known_hosts")
  revision=$(date +%Y%m%dT%H%M%S)
  remote="/home/builder/veil-warden-source-$revision"
  archive="$state/sandbox/source-$revision.tar.gz"
  COPYFILE_DISABLE=1 tar --no-xattrs -czf "$archive" -C "$repo" flake.nix flake.lock Cargo.toml Cargo.lock \
    rust-toolchain.toml .cargo crates config infra tests tools
  scp "${ssh_options[@]}" -P 32222 "$archive" builder@127.0.0.1:source.tar.gz
  ssh "${ssh_options[@]}" -p 32222 builder@127.0.0.1 \
    "set -eu; mkdir -p '$remote'; tar -xzf source.tar.gz -C '$remote'; nix --extra-experimental-features 'nix-command flakes' build 'path:$remote#packages.aarch64-linux.sandbox-bundle' --no-link --print-out-paths --max-jobs 2" \
    > "$state/sandbox/remote-build-path"
  bundle=$(cat "$state/sandbox/remote-build-path")
  destination="$state/sandbox/bundle-$revision"
  mkdir -p "$destination"
  scp "${ssh_options[@]}" -P 32222 "builder@127.0.0.1:$bundle/*" "$destination/"
  printf '%s\n' "$destination" > "$state/sandbox/build-path"
else
  nix build "path:$repo#packages.aarch64-linux.sandbox-bundle" --no-link \
    --print-out-paths --max-jobs 2 > "$state/sandbox/build-path"
fi
printf 'VM bundle built: %s\n' "$(cat "$state/sandbox/build-path")"
