#!/usr/bin/env bash
set -euo pipefail

# Cut and publish an ACE release end-to-end.
#
# Usage: ./release.sh <version>
#
# Bumps Cargo.toml/Cargo.lock/latest, cross-builds every target, patches the
# Homebrew formula, commits + tags, pushes, publishes the GitHub release, and
# pushes the homebrew-tap subtree — all in one linear flow with no in-between
# rebuild that could drift the formula sha away from the uploaded artifact.

if [ $# -ne 1 ]; then
  echo "Usage: ./release.sh <version>" >&2
  exit 1
fi

VERSION="${1#v}"
TAG="v$VERSION"
ARTIFACT="ace-aarch64-apple-darwin"
FORMULA="homebrew-tap/Formula/ace.rb"
BINARY="target/dist/$ARTIFACT"
URL="https://github.com/ace-rs/ace/releases/download/$TAG/$ARTIFACT"

if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "Error: working tree is dirty. Commit or stash changes first." >&2
  exit 1
fi

if ! cargo set-version --help >/dev/null 2>&1; then
  echo "Error: cargo set-version not found. Run: cargo install cargo-edit" >&2
  exit 1
fi

if [ ! -f "$FORMULA" ]; then
  echo "Error: $FORMULA not found." >&2
  exit 1
fi

echo "==> Bumping to $TAG"
cargo set-version "$VERSION"
echo "$TAG" > latest

# Build BEFORE the version-bump commit. build.rs embeds ACE_GIT_HASH from
# `git rev-parse --short HEAD` and uses cargo:rerun-if-changed=.git/HEAD, so
# any HEAD movement after this build would invalidate the formula sha.
# Building first means the binary embeds the pre-bump commit's hash (one
# behind the tag), but the formula sha and the uploaded artifact agree —
# which is the property users rely on.
echo "==> Building all targets"
./build-all.sh

if [ ! -f "$BINARY" ]; then
  echo "Error: $BINARY missing after build." >&2
  exit 1
fi

EXPECTED_SHA=$(shasum -a 256 "$BINARY" | cut -d' ' -f1)
echo "==> Patching formula (sha256: $EXPECTED_SHA)"

sed -i '' "s|^  version .*|  version \"$VERSION\"|" "$FORMULA"
sed -i '' "s|^  url .*|  url \"$URL\"|" "$FORMULA"
sed -i '' "s|^  sha256 .*|  sha256 \"$EXPECTED_SHA\"|" "$FORMULA"

git add Cargo.toml Cargo.lock latest "$FORMULA"
git commit -m "$TAG"
git tag "$TAG"

echo "==> Pushing to gh"
git push gh main
git push gh "$TAG"

echo "==> Publishing GitHub release"
gh release create "$TAG" \
  --title "ACE $TAG" \
  --generate-notes \
  target/dist/ace-*

echo "==> Verifying published artifact matches formula sha"
PUBLISHED_SHA=$(curl -fsSL "$URL" | shasum -a 256 | cut -d' ' -f1)
if [ "$PUBLISHED_SHA" != "$EXPECTED_SHA" ]; then
  echo "Error: published sha ($PUBLISHED_SHA) != expected ($EXPECTED_SHA)" >&2
  echo "  Release is broken — investigate before pushing the formula to gh-tap." >&2
  exit 1
fi
echo "==> sha verified: $EXPECTED_SHA"

if git remote get-url gh-tap >/dev/null 2>&1; then
  echo "==> Pushing formula to gh-tap"
  git subtree push --prefix=homebrew-tap gh-tap main
else
  echo "Warning: gh-tap remote not configured — skipping subtree push." >&2
  echo "  git remote add gh-tap gh:ace-rs/homebrew-tap" >&2
fi

echo
echo "==> Released: https://github.com/ace-rs/ace/releases/tag/$TAG"
