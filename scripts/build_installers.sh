#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")"/.. && pwd)"
VERSION=""
STAGE3_BIN=""
OUTPUT_DIR="$ROOT_DIR/artifacts/installers"
TARGETS=(linux)

usage() {
  cat <<'USAGE'
build_installers.sh - Produce platform installers from the Stage3 binary.

Usage: scripts/build_installers.sh [options]

Options:
  --version <ver>      Version used for installer metadata (required)
  --stage3 <path>      Path to Stage3 compiler binary (default: ./stage3_seen)
  --output-dir <dir>   Output directory for installers (default: artifacts/installers)
  --target <name>      Limit to specific target (repeatable; default: linux)
  -h, --help           Show this help message
USAGE
}

SELECTED=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="$2"
      shift 2
      ;;
    --stage3)
      STAGE3_BIN="$2"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --target)
      SELECTED+=("$2")
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

if [[ -z "$VERSION" ]]; then
  echo "--version is required" >&2
  exit 1
fi

if [[ -z "$STAGE3_BIN" ]]; then
  if [[ -x "$ROOT_DIR/stage3_seen" ]]; then
    STAGE3_BIN="$ROOT_DIR/stage3_seen"
  else
    echo "Stage3 binary missing; pass --stage3" >&2
    exit 1
  fi
fi

if [[ ! -x "$STAGE3_BIN" ]]; then
  echo "Stage3 binary not executable: $STAGE3_BIN" >&2
  exit 1
fi

mkdir -p "$OUTPUT_DIR"

if [[ ${#SELECTED[@]} -gt 0 ]]; then
  TARGETS=("${SELECTED[@]}")
fi

stage_linux() {
  local staging="$ROOT_DIR/installer/tmp/linux"
  rm -rf "$staging"
  mkdir -p "$staging"
  cp "$STAGE3_BIN" "$staging/seen"
  chmod +x "$staging/seen"
  echo "[installers] Building Linux DEB"
  (cd "$ROOT_DIR/installer/linux" && ./build-deb.sh "$VERSION" amd64 --source-dir installer/tmp/linux --output-dir "$OUTPUT_DIR/linux")
  echo "[installers] Building Linux RPM"
  (cd "$ROOT_DIR/installer/linux" && ./build-rpm.sh "$VERSION" x86_64 --source-dir installer/tmp/linux --output-dir "$OUTPUT_DIR/linux")
  echo "[installers] Building Linux AppImage"
  (cd "$ROOT_DIR/installer/linux" && ./build-appimage.sh "$VERSION" x86_64 --source-dir installer/tmp/linux --output-dir "$OUTPUT_DIR/linux")
}

stage_placeholder() {
  local name="$1"
  local file="$OUTPUT_DIR/${name}_README.txt"
  mkdir -p "$OUTPUT_DIR"
  cat > "$file" <<EOF_PLACEHOLDER
Installer build for $name is not yet automated.
Provision the target environment and run the platform-specific scripts under installer/ to produce artifacts.
EOF_PLACEHOLDER
}

for target in "${TARGETS[@]}"; do
  case "$target" in
    linux)
      stage_linux
      ;;
    windows|macos|android|ios|web)
      stage_placeholder "$target"
      ;;
    *)
      echo "Unknown target $target" >&2
      exit 1
      ;;
  esac
 done

echo "Installers written to $OUTPUT_DIR"
