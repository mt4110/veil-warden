#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && pwd)
state="${WARDEN_STATE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/veil-warden-m0}"
build=$(cat "$state/m1/build-path")
# Build a test-only failing-writer fixture inside the same pinned Linux toolchain.
builder_options=(-F /dev/null -o ConnectTimeout=5 -i "$state/private/operator" -o BatchMode=yes -o IdentitiesOnly=yes
  -o StrictHostKeyChecking=yes -o "UserKnownHostsFile=$state/bootstrap/known_hosts")
build_revision=${build##*/build-}
builder_dir="/home/builder/warden-m1-$build_revision"
ssh "${builder_options[@]}" -p 32222 builder@127.0.0.1 \
  "cd '$builder_dir'; export CARGO_TARGET_DIR=/home/builder/.cache/veil-warden-build/target; nix --extra-experimental-features 'nix-command flakes' develop 'path:$builder_dir' -c cargo test --locked -p veil-warden --no-run --message-format=json" \
  > "$build/test-build.json"
fixture=$(python3 -c 'import json,sys; print(next(r["executable"] for r in (json.loads(line) for line in open(sys.argv[1]) if line.startswith("{")) if r.get("executable") and r.get("profile",{}).get("test")))' "$build/test-build.json")
scp "${builder_options[@]}" -P 32222 "builder@127.0.0.1:$fixture" "$build/veil-warden-tests"
# Establish this boot's verified loopback SSH state using the existing helper.
"$repo/scripts/ssh-vm.sh" true
run=$(cat "$state/sandbox/current-run")
options=(-F /dev/null -o ConnectTimeout=5 -i "$state/private/operator" -o BatchMode=yes -o IdentitiesOnly=yes
  -o StrictHostKeyChecking=yes -o "UserKnownHostsFile=$run/known_hosts")
revision=$(date +%Y%m%dT%H%M%S)-$$
remote="/home/warden/warden-m4-$revision"
"$repo/scripts/ssh-vm.sh" "mkdir -p '$remote'"
scp "${options[@]}" -P 32223 "$build/veil-warden" "$build/veil-warden-ebpf" "$build/veil-warden-partial-fixture" \
  "$repo/tests/vm/cases/connect_monitor.py" "$repo/tests/vm/cases/connect_policy.py" "$repo/tests/vm/cases/tui_monitor.py" "$build/veil-warden-tests" "warden@127.0.0.1:$remote/"
mkdir -p "$repo/artifacts/m4"
"$repo/scripts/ssh-vm.sh" \
  "sudo python3 '$remote/tui_monitor.py' '$remote/veil-warden' '$remote/veil-warden-ebpf' '$remote/veil-warden-partial-fixture' '$remote/veil-warden-tests'" \
  > "$repo/artifacts/m4/acceptance-$revision.json"
cat "$repo/artifacts/m4/acceptance-$revision.json"
# SSH after all shutdown modes is itself a management-connection check.
"$repo/scripts/ssh-vm.sh" 'systemctl is-active sshd.service'
