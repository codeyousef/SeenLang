#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")"/.. && pwd)"
OUTPUT_DIR="$ROOT_DIR/artifacts/packages"
VERSION=""

usage() {
  cat <<'USAGE'
package_seen_std.sh - Produce deterministic archives for seen_std.

Usage: scripts/package_seen_std.sh [options]

Options:
  --version <ver>     Version label embedded in archive name (default: read from Seen.toml)
  --output-dir <dir>  Directory to place the archive (default: artifacts/packages)
  -h, --help          Show this help message
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="$2"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage
      exit 1
      ;;
  esac
 done

MANIFEST_PATH="$ROOT_DIR/seen_std/Seen.toml"
if [[ -z "$VERSION" ]]; then
  if [[ ! -f "$MANIFEST_PATH" ]]; then
    echo "Seen std manifest missing at $MANIFEST_PATH" >&2
    exit 1
  fi
  VERSION="$(python3 - <<'PY' "$MANIFEST_PATH" || true
import sys
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib
with open(sys.argv[1], 'rb') as fh:
    data = tomllib.load(fh)
print(data['project']['version'])
PY
)"
  if [[ -z "$VERSION" ]]; then
    echo "Unable to infer seen_std version from $MANIFEST_PATH; pass --version explicitly" >&2
    exit 1
  fi
fi

CLI_BIN="${CARGO_TARGET_BIN:-${CARGO_TARGET_DIR:-$HOME/.cargo/target-shared}/release}/seen_cli"
if [[ ! -x "$CLI_BIN" ]]; then
    echo "seen_cli not found at $CLI_BIN" >&2
    echo "Build it with: cargo build -p seen_cli --release" >&2
    exit 1
fi

PACKAGE_NAME="libseen_std-${VERSION}.seenpkg"
OUTPUT_PATH="$OUTPUT_DIR/$PACKAGE_NAME"
CHECKSUM_PATH="$OUTPUT_PATH.sha256"

mkdir -p "$OUTPUT_DIR"

"$CLI_BIN" pkg "$ROOT_DIR/seen_std" --output "$OUTPUT_PATH" --require-lock

sha256sum "$OUTPUT_PATH" > "$CHECKSUM_PATH"

echo "Packaged seen_std -> $OUTPUT_PATH"
