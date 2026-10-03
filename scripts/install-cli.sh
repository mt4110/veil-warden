#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
set -euo pipefail
umask 077
repo=$(cd "$(dirname "$0")/.." && pwd)
if [[ "$(uname -s)" != Darwin || "$(uname -m)" != arm64 ]]; then
  echo 'The host CLI requires an Apple Silicon Mac.' >&2
  exit 1
fi
nix_path=$(command -v nix)
bin="$HOME/.local/bin"
mkdir -p "$repo/target/host-cli" "$bin"
export WARDEN_REPO="$repo" WARDEN_NIX="$nix_path"
nix --extra-experimental-features 'nix-command flakes' develop "path:$repo" --command \
  rustc --edition=2024 -D warnings -O "$repo/tools/host-cli/main.rs" -o "$repo/target/host-cli/veil-warden"
# Preserve an existing installation before replacing it; no files are deleted.
if [[ -e "$bin/veil-warden" ]]; then
  cp -p "$bin/veil-warden" "$bin/veil-warden.backup-$(date +%Y%m%dT%H%M%S)-$$"
fi
cp "$repo/target/host-cli/veil-warden" "$bin/veil-warden"
python3 - <<'PY'
from pathlib import Path
import os
rc = Path(os.environ.get('ZDOTDIR', str(Path.home()))) / '.zshrc'
line = 'export PATH="$HOME/.local/bin:$PATH" # veil-warden CLI'
text = rc.read_text() if rc.exists() else ''
if line not in text.splitlines():
    with rc.open('a') as f:
        f.write(('\n' if text and not text.endswith('\n') else '') + '\n' + line + '\n')
print(f'Installed ~/.local/bin/veil-warden; PATH added to {rc}')
PY
printf 'Next: open a new terminal and run veil-warden (or: %s/veil-warden)\n' "$bin"
