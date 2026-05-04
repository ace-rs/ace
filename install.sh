#!/usr/bin/env bash
set -euo pipefail

# Install ACE from GitHub releases.
#
# Usage:
#   curl -fsSL https://ace-rs.dev/install.sh | bash
#
# Installs the latest release binary to ~/.local/bin/ace.

REPO="ace-rs/ace"
LATEST_URL="https://ace-rs.dev/latest"
INSTALL_DIR="${HOME}/.local/bin"

# --- Detect platform ----------------------------------------------------------

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin) TRIPLE_OS="apple-darwin" ;;
  Linux)  TRIPLE_OS="unknown-linux-gnu" ;;
  *)
    echo "Error: unsupported OS: $OS"
    exit 1
    ;;
esac

case "$ARCH" in
  aarch64|arm64) TRIPLE_ARCH="aarch64" ;;
  x86_64)        TRIPLE_ARCH="x86_64" ;;
  *)
    echo "Error: unsupported architecture: $ARCH"
    exit 1
    ;;
esac

TARGET="${TRIPLE_ARCH}-${TRIPLE_OS}"

# --- Resolve latest release ---------------------------------------------------

echo "Fetching latest release..."
TAG="$(curl -fsSL "$LATEST_URL" | tr -d '[:space:]')"

if [ -z "$TAG" ]; then
  echo "Error: could not determine latest release tag from ${LATEST_URL}."
  exit 1
fi

# --- Download binary ----------------------------------------------------------

ASSET_URL="https://github.com/${REPO}/releases/download/${TAG}/ace-${TARGET}"
TMPFILE="$(mktemp)"
trap 'rm -f "$TMPFILE"' EXIT

echo "Downloading ace ${TAG} (${TARGET})..."
curl -fsSL -o "$TMPFILE" "$ASSET_URL"

if [ ! -s "$TMPFILE" ]; then
  echo "Error: download failed or produced empty file."
  exit 1
fi

# --- Install ------------------------------------------------------------------

chmod +x "$TMPFILE"
mkdir -p "$INSTALL_DIR"
mv "$TMPFILE" "${INSTALL_DIR}/ace"

echo "Installed ace ${TAG} to ${INSTALL_DIR}/ace"

if ! echo ":${PATH}:" | grep -q ":${INSTALL_DIR}:"; then
  echo ""
  echo "Note: ${INSTALL_DIR} is not on your PATH."
  echo "Add it with:  export PATH=\"${INSTALL_DIR}:\$PATH\""
fi
