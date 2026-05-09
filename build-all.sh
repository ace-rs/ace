#!/usr/bin/env bash
set -euo pipefail

# Cross-build ACE for all release targets using cargo-zigbuild.
#
# Runs on macOS or Linux:
#   macOS  — builds all targets (darwin + linux)
#   Linux  — builds linux targets only (darwin targets require macOS runners)
#
# Prerequisites:
#   cargo install cargo-zigbuild
#   zig 0.14.x or 0.15.2 — https://ziglang.org/download/
#
# Zig version notes:
#   - 0.16 has a known `ar` regression that breaks `ring` and other crates:
#     rust-cross/cargo-zigbuild#433. Use 0.14.x or 0.15.2 instead.
#   - 0.13 / earlier may also work but are not actively tested.
#   `brew install zig` currently pulls 0.16 — install a known-good version
#   manually from the ziglang.org archives if your package manager is too new.
#
# macOS cross-compilation note:
#   *-apple-darwin targets are built with plain `cargo build` (Apple's
#   toolchain handles arm64<->x86_64 natively when the rust target is
#   installed). Zig 0.14 cannot resolve Apple frameworks (e.g. `objc`)
#   when SDKROOT is set — its sysroot doesn't understand Apple's SDK
#   layout — so we never invoke zigbuild for darwin targets and only
#   export SDKROOT along the cargo-build path.

TARGETS=(
  aarch64-apple-darwin
  x86_64-apple-darwin
  aarch64-unknown-linux-gnu
  x86_64-unknown-linux-gnu
  aarch64-unknown-linux-musl
  x86_64-unknown-linux-musl
  x86_64-pc-windows-gnu
)

OUTDIR="${1:-target/dist}"
mkdir -p "$OUTDIR"

HOST_OS="$(uname -s)"

# --- Filter targets by host OS ----------------------------------------------

darwin_targets=()
zig_targets=()
skipped=()

for target in "${TARGETS[@]}"; do
  case "$target" in
    *-apple-darwin)
      if [ "$HOST_OS" = "Darwin" ]; then
        darwin_targets+=("$target")
      else
        skipped+=("$target")
      fi
      ;;
    *)
      zig_targets+=("$target")
      ;;
  esac
done

build_targets=("${darwin_targets[@]}" "${zig_targets[@]}")

if [ ${#skipped[@]} -gt 0 ]; then
  echo "Skipping (need macOS runner): ${skipped[*]}"
fi

if [ ${#build_targets[@]} -eq 0 ]; then
  echo "Error: no buildable targets for this platform."
  exit 1
fi

# --- Preflight checks -------------------------------------------------------

if ! command -v cargo-zigbuild &>/dev/null; then
  echo "Error: cargo-zigbuild not found."
  echo ""
  echo "  cargo install cargo-zigbuild"
  echo ""
  exit 1
fi

if ! command -v zig &>/dev/null; then
  echo "Error: zig not found."
  echo ""
  echo "  See https://ziglang.org/download/"
  echo ""
  exit 1
fi

# Optional: sccache speeds up repeat builds by caching rustc invocations
# across targets and runs. Detect-and-enable; print a hint if absent.
if command -v sccache &>/dev/null; then
  export RUSTC_WRAPPER=sccache
  echo "Using sccache: $(sccache --version 2>&1 | head -1)"
else
  echo "Hint: install sccache for faster repeat builds — cargo install sccache"
fi

# Resolve SDKROOT for the cargo-build (Apple toolchain) path on macOS.
# NOT exported globally — zigbuild on darwin targets breaks if SDKROOT is set
# (zig 0.14 can't resolve Apple frameworks like `objc` through its own sysroot).
# We pass SDKROOT only into the `cargo build` invocations for *-apple-darwin.
SDKROOT_RESOLVED=""
if [ "$HOST_OS" = "Darwin" ]; then
  SDKROOT_RESOLVED="${SDKROOT:-$(xcrun --show-sdk-path 2>/dev/null || true)}"
  if [ -z "$SDKROOT_RESOLVED" ]; then
    echo "Error: SDKROOT not set and xcrun failed."
    exit 1
  fi
  unset SDKROOT
fi

# Ensure all required Rust targets are installed.
for target in "${build_targets[@]}"; do
  if ! rustup target list --installed | grep -qx "$target"; then
    echo "Installing Rust target: $target"
    rustup target add "$target"
  fi
done

# --- Build targets -----------------------------------------------------------
#
# Each group is built with a single multi-target invocation so cargo walks
# the dep graph once and shares artifact resolution across targets. On group
# failure we re-run per-target to surface which one broke (a fused error
# can't tell us).

failed=()

# Build a target group: $1 = label, $2 = "darwin"|"zig", remaining args = targets.
# Each target has its own target/<triple>/ subdir, so output collisions are impossible.
build_group() {
  local label="$1" mode="$2"
  shift 2
  local -a targets=("$@")
  [ ${#targets[@]} -gt 0 ] || return 0

  echo "==> $label group: ${targets[*]}"

  local -a target_flags=()
  for t in "${targets[@]}"; do
    target_flags+=(--target "$t")
  done

  local -a cmd
  case "$mode" in
    darwin) cmd=(env SDKROOT="$SDKROOT_RESOLVED" cargo build --release "${target_flags[@]}") ;;
    zig)    cmd=(cargo zigbuild --release "${target_flags[@]}") ;;
  esac

  if "${cmd[@]}"; then
    return 0
  fi

  echo "    !! group build failed — retrying per-target to isolate"
  for t in "${targets[@]}"; do
    case "$mode" in
      darwin) cmd=(env SDKROOT="$SDKROOT_RESOLVED" cargo build --release --target "$t") ;;
      zig)    cmd=(cargo zigbuild --release --target "$t") ;;
    esac
    if ! "${cmd[@]}"; then
      echo "    !! FAILED: $t"
      failed+=("$t")
    fi
  done
}

build_group "darwin" darwin "${darwin_targets[@]}"
build_group "zig"    zig    "${zig_targets[@]}"

# Collect outputs for every target the build actually produced.
for target in "${build_targets[@]}"; do
  case "$target" in
    *-windows-*)
      bin="target/$target/release/ace.exe"
      out="$OUTDIR/ace-$target.exe"
      ;;
    *)
      bin="target/$target/release/ace"
      out="$OUTDIR/ace-$target"
      ;;
  esac
  if [ -f "$bin" ]; then
    cp "$bin" "$out"
    echo "    -> $out"
  fi
done

echo ""
echo "--- Results ---"
ls -lh "$OUTDIR"/ace-* 2>/dev/null || echo "(no binaries produced)"

if [ ${#failed[@]} -gt 0 ]; then
  echo ""
  echo "Failed targets:"
  printf "  %s\n" "${failed[@]}"
  exit 1
fi
