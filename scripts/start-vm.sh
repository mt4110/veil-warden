#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && pwd)
state="${WARDEN_STATE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/veil-warden-m0}"
log="$state/sandbox/quickstart-vm.log"

if [[ "$(uname -s)" != Darwin || "$(uname -m)" != arm64 ]]; then
  echo "This helper requires an Apple Silicon Mac." >&2
  exit 1
fi
"$repo/scripts/build-vm.sh"
mkdir -p "$(dirname "$log")"
"$repo/scripts/run-vm.sh" </dev/null >"$log" 2>&1 &
vm_pid=$!

printf 'Waiting for the sandbox preflight'
for _ in $(seq 1 120); do
  if ! kill -0 "$vm_pid" 2>/dev/null; then
    echo >&2
    echo "Sandbox VM exited. See $log" >&2
    exit 1
  fi
  if "$repo/scripts/ssh-vm.sh" 'sudo warden-preflight' > "$state/sandbox/preflight.json" 2>/dev/null; then
    echo
    cat "$state/sandbox/preflight.json"
    printf 'Sandbox is ready. VM output: %s\n' "$log"
    printf "Stop it with: ./scripts/ssh-vm.sh 'sudo poweroff'\n"
    exit 0
  fi
  printf '.'
  sleep 1
done
echo >&2
echo "Sandbox did not pass preflight within 120 seconds. See $log" >&2
exit 1
