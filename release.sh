#!/usr/bin/env bash
set -euo pipefail

# Release ACE binaries to GitHub.
#
# Prerequisites:
#   1. ./bump.sh 0.2.0    (bumps Cargo.toml, Cargo.lock, latest, commits, tags)
#   2. git push gh main && git push gh v0.2.0
#   3. ./release.sh

VERSION="$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')"
TAG="v$VERSION"

# Verify tag exists on HEAD.
HEAD_TAGS="$(git tag --points-at HEAD)"
if ! echo "$HEAD_TAGS" | grep -qx "$TAG"; then
  echo "Error: tag $TAG not found on HEAD."
  echo ""
  echo "  cargo set-version $VERSION"
  echo "  git commit -am \"Bump version to $VERSION\""
  echo "  git tag $TAG"
  echo ""
  exit 1
fi

# Refuse to release with uncommitted changes (formula update happens post-upload).
if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "Error: working tree is dirty. Commit or stash changes first."
  exit 1
fi

FORMULA="homebrew-tap/Formula/ace.rb"
if [ ! -f "$FORMULA" ]; then
  echo "Error: $FORMULA not found."
  exit 1
fi

echo "==> Releasing ACE $TAG"

# Build all targets.
./build-all.sh

# Create GitHub release with binaries.
DIST="target/dist"
gh release create "$TAG" \
  --title "ACE $TAG" \
  --generate-notes \
  "$DIST"/ace-*

# --- Update Homebrew formula ---------------------------------------------------

BINARY="$DIST/ace-aarch64-apple-darwin"
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

git add "$FORMULA"
git commit -m "homebrew: update formula to $TAG"

# Push formula to the tap repo via subtree.
if git remote get-url gh-tap &>/dev/null; then
  git subtree push --prefix=homebrew-tap gh-tap main
  echo "==> Pushed Homebrew formula to gh-tap"
else
  echo "Warning: gh-tap remote not configured — skipping subtree push."
  echo "  git remote add gh-tap gh:ace-rs/homebrew-tap"
fi

echo ""
echo "==> Released: https://github.com/ace-rs/ace/releases/tag/$TAG"
