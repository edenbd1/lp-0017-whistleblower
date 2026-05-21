#!/usr/bin/env bash
# scripts/ci-local.sh — mirror the fast tier of `.github/workflows/ci.yml`
# locally. Run before pushing to avoid embarrassing CI red.

set -euo pipefail

echo "▶ cargo fmt --check"
cargo fmt --all -- --check

echo "▶ cargo clippy (workspace)"
cargo clippy --workspace --all-targets -- -D warnings

echo "▶ cargo test (workspace, excl. guest)"
cargo test --workspace --exclude whistleblower-registry-guest

echo
echo "all green — safe to push"
