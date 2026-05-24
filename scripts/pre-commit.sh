#!/usr/bin/env bash
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

echo "[pre-commit] Check Rust formatting"
cargo fmt -- --check

echo "[pre-commit] Run Clippy"
cargo clippy --all-targets -- -D warnings
