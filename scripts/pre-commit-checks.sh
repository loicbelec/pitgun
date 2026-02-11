#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"

echo "[1/4] cargo build --all --verbose"
cargo build --all --verbose

echo "[2/4] cargo test --all --verbose"
cargo test --all --verbose

echo "[3/4] cargo fmt -p pitgun-gateway -p pitgun-authority -- --check"
cargo fmt -p pitgun-gateway -p pitgun-authority -- --check

echo "[4/4] cargo clippy -p pitgun-gateway -p pitgun-authority -- -D warnings"
cargo clippy -p pitgun-gateway -p pitgun-authority -- -D warnings

echo "All pre-commit checks passed."
