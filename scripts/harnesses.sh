#!/bin/sh
#
# Provision real backend CLIs from source into a disposable, repo-local pen.
#
#   ./scripts/harnesses.sh            # status of every known backend
#   ./scripts/harnesses.sh hermes     # clone-or-pull, build, link .harnesses/bin/hermes
#   ./scripts/harnesses.sh clean      # remove the pen
#
# The pen is gitignored and never on `cargo test`'s path. See
# docs/guides/harnesses.md.

set -e

root=$(cd "$(dirname "$0")/.." && pwd)
pen="$root/.harnesses"
bin="$pen/bin"

# name | git url | build command | binary path inside the clone
backends() {
  cat <<'TABLE'
hermes|https://github.com/NousResearch/hermes-agent|uv sync|.venv/bin/hermes
codex|https://github.com/openai/codex|cargo build --release --manifest-path codex-rs/Cargo.toml|codex-rs/target/release/codex
opencode|https://github.com/sst/opencode|bun install|packages/opencode/bin/opencode
TABLE
}

lookup() {
  backends | grep "^$1|" || true
}

status() {
  backends | while IFS='|' read -r name url build binpath; do
    if [ -x "$bin/$name" ]; then
      echo "$name	built	$bin/$name"
    elif [ -d "$pen/$name" ]; then
      echo "$name	cloned	$pen/$name"
    else
      echo "$name	absent	$url"
    fi
  done
}

provision() {
  entry=$(lookup "$1")
  if [ -z "$entry" ]; then
    echo "unknown backend: $1" >&2
    echo "known: $(backends | cut -d'|' -f1 | tr '\n' ' ')" >&2
    exit 2
  fi

  name=$(echo "$entry" | cut -d'|' -f1)
  url=$(echo "$entry" | cut -d'|' -f2)
  build=$(echo "$entry" | cut -d'|' -f3)
  binpath=$(echo "$entry" | cut -d'|' -f4)
  clone="$pen/$name"

  if [ -d "$clone/.git" ]; then
    echo "==> pulling $name"
    git -C "$clone" pull --ff-only
  else
    echo "==> cloning $name from $url"
    mkdir -p "$pen"
    git clone --depth 1 "$url" "$clone"
  fi

  echo "==> building $name: $build"
  (cd "$clone" && nice -n 19 sh -c "$build")

  if [ ! -x "$clone/$binpath" ]; then
    echo "build finished but $clone/$binpath is not executable" >&2
    exit 1
  fi

  mkdir -p "$bin"
  ln -sf "$clone/$binpath" "$bin/$name"
  echo "==> $bin/$name"
}

case "$1" in
  "")
    status
    ;;
  clean)
    if [ -d "$pen" ]; then
      rm -rf "$pen"
      echo "removed $pen"
    else
      echo "nothing to clean"
    fi
    ;;
  *)
    provision "$1"
    ;;
esac
