#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
set -euo pipefail
state="${WARDEN_STATE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/veil-warden-m0}"
run=$(cat "$state/sandbox/current-run")
if [[ ! -s "$run/known_hosts" ]]; then
  # Loopback-only TOFU for this new ephemeral boot; hostile local users are out of scope.
  ssh-keyscan -T 5 -t ed25519 -p 32223 127.0.0.1 > "$run/known_hosts"
  test -s "$run/known_hosts"
fi
exec ssh -F /dev/null -o ConnectTimeout=5 -i "$state/private/operator" -p 32223 \
  -o BatchMode=yes -o IdentitiesOnly=yes -o StrictHostKeyChecking=yes \
  -o UserKnownHostsFile="$run/known_hosts" warden@127.0.0.1 "$@"
