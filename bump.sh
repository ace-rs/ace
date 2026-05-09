#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:?Usage: ./bump.sh <version>}"
VERSION="${VERSION#v}"
TAG="v$VERSION"

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

FORMULA="homebrew-tap/Formula/ace.rb"
if [ ! -f "$FORMULA" ]; then
  echo "Error: $FORMULA not found."
  exit 1
fi

cargo set-version "$VERSION"
cargo build --quiet 2>/dev/null || true
echo "$TAG" > latest

# Build all targets — needed now to compute the macOS aarch64 sha for the
# Homebrew formula, so the formula update lands in the same commit/tag as
# the version bump (no follow-up "homebrew: update formula" commit).
./build-all.sh

BINARY="target/dist/ace-aarch64-apple-darwin"
if [ ! -f "$BINARY" ]; then
  echo "Error: $BINARY not found — cannot update Homebrew formula."
  exit 1
fi

SHA="$(shasum -a 256 "$BINARY" | cut -d' ' -f1)"
echo "==> Updating Homebrew formula (sha256: $SHA)"

sed -i '' \
  -e "s|^  version \".*\"|  version \"$VERSION\"|" \
  -e "s|^  url \".*\"|  url \"https://github.com/ace-rs/ace/releases/download/$TAG/ace-aarch64-apple-darwin\"|" \
  -e "s|^  sha256 \".*\"|  sha256 \"$SHA\"|" \
  "$FORMULA"

git add Cargo.toml Cargo.lock latest "$FORMULA"
git commit -m "$TAG"
git tag "$TAG"

echo "==> Tagged $TAG with formula update — run ./release.sh to push and publish"
