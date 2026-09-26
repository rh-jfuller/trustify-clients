#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cargo run --locked --manifest-path "$repo_root/rust/Cargo.toml" -p xtask -- generate-api
cargo fmt --manifest-path "$repo_root/rust/Cargo.toml" --all
