#!/usr/bin/env bash
set -euo pipefail

# Release ACE binaries to GitHub.
#
# Prerequisites:
#   1. ./bump.sh 0.2.0    (bumps Cargo.toml/Cargo.lock/latest, builds, patches
#                          formula, commits + tags)
#   2. ./release.sh       (rebuilds (cached), pushes, publishes, subtree push)

VERSION="$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')"
TAG="v$VERSION"

# Verify tag exists on HEAD.
HEAD_TAGS="$(git tag --points-at HEAD)"
if ! echo "$HEAD_TAGS" | grep -qx "$TAG"; then
  echo "Error: tag $TAG not found on HEAD — run ./bump.sh $VERSION first."
  exit 1
fi

# Refuse to release with uncommitted changes.
if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "Error: working tree is dirty. Commit or stash changes first."
  exit 1
fi

echo "==> Releasing ACE $TAG"

# Re-run build (cached no-op if bump.sh already built everything).
./build-all.sh

DIST="target/dist"

# Push commits + tag before creating the GitHub release so the tag exists
# remotely.
git push gh main
git push gh "$TAG"

# Create GitHub release with binaries.
gh release create "$TAG" \
  --title "ACE $TAG" \
  --generate-notes \
  "$DIST"/ace-*

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
