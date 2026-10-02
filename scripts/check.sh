#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
python3 -m unittest discover -s tests/unit -v
markdownlint-cli2
nixfmt --check flake.nix infra/nixos/vm.nix
shellcheck scripts/*.sh
nix flake check "path:$PWD" --no-build
