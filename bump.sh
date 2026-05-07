#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:?Usage: ./bump.sh <version>}"
VERSION="${VERSION#v}"

# Refuse to bump with uncommitted changes.
if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "Error: working tree is dirty. Commit or stash changes first."
  exit 1
fi

if ! cargo set-version --help &>/dev/null; then
  echo "Error: cargo set-version not found."
  echo ""
  echo "  cargo install cargo-edit"
  echo ""
  exit 1
fi

cargo set-version "$VERSION"
cargo build --quiet 2>/dev/null || true
echo "v$VERSION" > latest
git add Cargo.toml Cargo.lock latest
git commit -m "v$VERSION"
git tag "v$VERSION"

echo "==> Tagged v$VERSION — run ./release.sh to publish"
