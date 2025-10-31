#!/usr/bin/env bash

# Pre-commit hook for StarRocks Admin
# Following rustfs standard: fmt + clippy + check

set -e

# Get project root (pre-commit.sh is in build directory)
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_DIR="$PROJECT_ROOT/build"

echo "[pre-commit] Running pre-commit checks..."
cd "$PROJECT_ROOT/backend"

# 1. Format code
echo "[pre-commit] Formatting code..."
cargo fmt --all

# 2. Run clippy (fix + strict check)
echo "[pre-commit] Running clippy checks..."
DATABASE_URL="sqlite:$BUILD_DIR/data/starrocks-admin.db" cargo clippy --fix --allow-dirty --allow-staged --allow-no-vcs --all-targets
DATABASE_URL="sqlite:$BUILD_DIR/data/starrocks-admin.db" cargo clippy --all-targets --all-features -- -D warnings

# 3. Run cargo check
echo "[pre-commit] Running cargo check..."
cargo check --all-targets

echo "[pre-commit] All pre-commit checks passed!"