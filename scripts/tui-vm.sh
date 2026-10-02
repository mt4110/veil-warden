#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && pwd)
state="${WARDEN_STATE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/veil-warden-m0}"
enforce=""
if [[ $# == 1 && "$1" == --enforce ]]; then
  enforce="--enforce"
elif [[ $# != 0 ]]; then
  echo 'usage: tui-vm.sh [--enforce]' >&2
  exit 1
fi
if [[ ! -t 0 || ! -t 1 ]]; then
  echo 'Run tui-vm.sh from an interactive terminal.' >&2
  exit 1
fi
build=$(cat "$state/m1/build-path")
"$repo/scripts/ssh-vm.sh" true
run=$(cat "$state/sandbox/current-run")
options=(-i "$state/private/operator" -o BatchMode=yes -o IdentitiesOnly=yes
  -o StrictHostKeyChecking=yes -o "UserKnownHostsFile=$run/known_hosts")
revision=$(date +%Y%m%dT%H%M%S)-$$
remote="/home/warden/warden-tui-$revision"
"$repo/scripts/ssh-vm.sh" "mkdir -p '$remote'"
scp "${options[@]}" -P 32223 "$build/veil-warden" "$build/veil-warden-ebpf" "warden@127.0.0.1:$remote/"
ssh "${options[@]}" -t -p 32223 warden@127.0.0.1 \
  "sudo systemctl start warden-test.slice && sudo '$remote/veil-warden' connect --object '$remote/veil-warden-ebpf' --tui $enforce"
