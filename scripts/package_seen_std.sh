#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")"/.. && pwd)"
OUTPUT_DIR="$ROOT_DIR/artifacts/packages"
VERSION="dev"
SNAPSHOT_ONLY=0

usage() {
  cat <<'USAGE'
package_seen_std.sh - Produce deterministic archives for seen_std.

Usage: scripts/package_seen_std.sh [options]

Options:
  --version <ver>     Version label embedded in archive name (default: dev)
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

PACKAGE_NAME="seen_std-${VERSION}.tar.gz"
OUTPUT_PATH="$OUTPUT_DIR/$PACKAGE_NAME"
CHECKSUM_PATH="$OUTPUT_PATH.sha256"

mkdir -p "$OUTPUT_DIR"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

cp -R "$ROOT_DIR/seen_std" "$TMP_DIR/seen_std"

TZ=UTC find "$TMP_DIR/seen_std" -print0 | xargs -0 touch -t 202401010000

tar --sort=name \
    --mtime='UTC 2024-01-01 00:00:00' \
    --owner=0 --group=0 --numeric-owner \
    -czf "$OUTPUT_PATH" -C "$TMP_DIR" seen_std

sha256sum "$OUTPUT_PATH" > "$CHECKSUM_PATH"

echo "Packaged seen_std -> $OUTPUT_PATH"
