#!/usr/bin/env bash
set -euo pipefail

# Release ACE binaries to GitHub.
#
# Prerequisites:
#   0. Clean tree:    git status must be clean — no uncommitted or staged
#                     changes. Commit or stash everything BEFORE bumping.
#   1. Bump version:  cargo set-version 0.2.0  (or --bump patch/minor/major)
#                     Install: cargo install cargo-edit
#   2. Update latest: echo "v0.2.0" > latest
#   3. Commit:        git commit -am "Bump version to 0.2.0"
#   4. Push:          git push gh main
#   5. Tag + push:    git tag v0.2.0 && git push gh v0.2.0
#   6. Run:           ./release.sh

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

# Refuse to release with uncommitted changes.
if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "Error: working tree is dirty. Commit or stash changes first."
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

echo ""
echo "==> Released: https://github.com/ace-rs/ace/releases/tag/$TAG"
