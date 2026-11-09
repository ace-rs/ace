#!/usr/bin/env bash
set -euo pipefail

# Cut and publish an ACE release end-to-end.
#
# Usage: ./release.sh <version>
#
# Bumps Cargo.toml/Cargo.lock/latest, cross-builds every target, commits + tags,
# pushes, publishes the GitHub release, verifies the published macOS arm64
# artifact against the built one, and prints the Homebrew formula values for
# the tap's owner. The tap (ace-rs/homebrew-tap) is a separate repo; this
# script never writes to it.

if [ $# -ne 1 ]; then
  echo "Usage: ./release.sh <version>" >&2
  exit 1
fi

VERSION="${1#v}"
TAG="v$VERSION"
ARTIFACT="ace-aarch64-apple-darwin"
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

echo "==> Bumping to $TAG"
cargo set-version "$VERSION"
echo "$TAG" > latest

# Build BEFORE the version-bump commit. build.rs embeds ACE_GIT_HASH from
# `git rev-parse --short HEAD` and uses cargo:rerun-if-changed=.git/HEAD, so
# any HEAD movement after this build would rebuild a binary whose sha differs
# from the one uploaded. Building first means the binary embeds the pre-bump
# commit's hash (one behind the tag), but the formula sha and the uploaded
# artifact agree — which is the property Homebrew users rely on.
echo "==> Building all targets"
./build-all.sh

if [ ! -f "$BINARY" ]; then
  echo "Error: $BINARY missing after build." >&2
  exit 1
fi

EXPECTED_SHA=$(shasum -a 256 "$BINARY" | cut -d' ' -f1)

git add Cargo.toml Cargo.lock latest
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

echo "==> Verifying published artifact matches the built sha"
PUBLISHED_SHA=$(curl -fsSL "$URL" | shasum -a 256 | cut -d' ' -f1)
if [ "$PUBLISHED_SHA" != "$EXPECTED_SHA" ]; then
  echo "Error: published sha ($PUBLISHED_SHA) != built ($EXPECTED_SHA)" >&2
  echo "  Release is broken — investigate before handing the formula to the tap." >&2
  exit 1
fi

echo
echo "==> Released: https://github.com/ace-rs/ace/releases/tag/$TAG"
echo "==> Hand these Formula/ace.rb values to the ace-rs/homebrew-tap owner:"
echo "  version \"$VERSION\""
echo "  url \"$URL\""
echo "  sha256 \"$EXPECTED_SHA\""
