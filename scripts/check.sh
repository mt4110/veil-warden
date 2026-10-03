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
python3 -m unittest discover -s tests/unit -v
markdownlint-cli2
nixfmt --check flake.nix infra/nixos/vm.nix
shellcheck scripts/*.sh
nix flake check "path:$PWD" --no-build
