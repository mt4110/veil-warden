#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && pwd)
state="${WARDEN_STATE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/veil-warden-m0}"
build=$(cat "$state/m1/build-path")
"$repo/scripts/ssh-vm.sh" true
run=$(cat "$state/sandbox/current-run")
options=(-i "$state/private/operator" -o BatchMode=yes -o IdentitiesOnly=yes
  -o StrictHostKeyChecking=yes -o "UserKnownHostsFile=$run/known_hosts")
revision=$(date +%Y%m%dT%H%M%S)-$$
remote="/home/warden/warden-m5-$revision"
"$repo/scripts/ssh-vm.sh" "mkdir -p '$remote'"
scp "${options[@]}" -P 32223 "$build/veil-warden" "$build/veil-warden-ebpf" \
  "$repo/tests/vm/cases/connect_monitor.py" "$repo/tests/vm/cases/connect_policy.py" \
  "$repo/tests/vm/cases/tui_monitor.py" "$repo/tests/vm/cases/secret_warning.py" "warden@127.0.0.1:$remote/"
mkdir -p "$repo/artifacts/m5"
"$repo/scripts/ssh-vm.sh" "sudo python3 '$remote/secret_warning.py' '$remote/veil-warden' '$remote/veil-warden-ebpf'" \
  > "$repo/artifacts/m5/acceptance-$revision.json"
cat "$repo/artifacts/m5/acceptance-$revision.json"
