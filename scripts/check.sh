#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all -- --check
rustfmt --edition 2024 --check tools/host-cli/main.rs
mkdir -p target/host-cli
WARDEN_REPO="$PWD" WARDEN_NIX="$(command -v nix)" rustc --edition=2024 -D warnings \
  --test tools/host-cli/main.rs -o target/host-cli/tests
./target/host-cli/tests
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
if [[ "$(uname -s)" == Darwin && "$(uname -m)" == arm64 ]]; then
  WARDEN_REPO="$PWD" WARDEN_NIX="$(command -v nix)" rustc --edition=2024 -D warnings \
    tools/host-cli/main.rs -o target/host-cli/veil-warden
  cargo build --locked -p veil-warden --bin veil-warden
  cp target/debug/veil-warden target/host-cli/veil-warden-preview
  python3 tests/host/test_dry_run.py target/host-cli/veil-warden
fi
python3 -m unittest discover -s tests/unit -v
markdownlint-cli2
nixfmt --check flake.nix infra/nixos/vm.nix
shellcheck scripts/*.sh
nix flake check "path:$PWD" --no-build
