#!/usr/bin/env sh
# Run the same checks as GitHub Actions CI (.github/workflows/ci.yml).
set -eu

ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
cd "$ROOT"

echo "==> cargo fmt --check"
cargo fmt --all -- --check

echo "==> cargo build --locked"
cargo build --locked

echo "==> cargo clippy (-D warnings)"
cargo clippy --all-targets --locked -- -D warnings

echo "==> cargo test --locked"
cargo test --locked

if ! command -v cargo-audit >/dev/null 2>&1; then
  echo "==> installing cargo-audit"
  cargo install cargo-audit --locked
fi

echo "==> cargo audit"
cargo audit

echo "CI checks passed."
