#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")"/.. && pwd)"
OUTPUT_DIR="$ROOT_DIR/artifacts/platform-matrix"
STAGE3_BIN=""
TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
PLATFORMS=(
  "linux-x86_64"
  "windows-x86_64"
  "macos-arm64"
  "android-arm64"
  "ios-arm64"
  "web-wasm32"
)

usage() {
  cat <<'USAGE'
platform_matrix.sh - Smoke test Stage3 toolchain across supported platforms.

Usage: scripts/platform_matrix.sh [options]

Options:
  --stage3 <path>      Path to Stage3 Seen compiler (default: ./stage3_seen if present)
  --output-dir <dir>   Directory to store JSON reports (default: artifacts/platform-matrix)
  --platform <name>    Limit run to a single platform (repeatable)
  -h, --help           Show this help message
USAGE
}

SELECTED=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --stage3)
      STAGE3_BIN="$2"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --platform)
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

if [[ -z "$STAGE3_BIN" ]]; then
  if [[ -x "$ROOT_DIR/stage3_seen" ]]; then
    STAGE3_BIN="$ROOT_DIR/stage3_seen"
  else
    STAGE3_BIN="$(command -v seen_cli || true)"
    if [[ -z "$STAGE3_BIN" ]]; then
      echo "stage3_seen not found and seen_cli unavailable; pass --stage3" >&2
      exit 1
    fi
    echo "[matrix] stage3 binary missing; falling back to $STAGE3_BIN (development mode)"
  fi
fi

mkdir -p "$OUTPUT_DIR/$TIMESTAMP"
REPORT_DIR="$OUTPUT_DIR/$TIMESTAMP"

declare -a TARGET_PLATFORMS
if [[ ${#SELECTED[@]} -gt 0 ]]; then
  TARGET_PLATFORMS=("${SELECTED[@]}")
else
  TARGET_PLATFORMS=("${PLATFORMS[@]}")
fi

run_linux() {
  local platform="$1"
  local report="$2"
  local ecs_output="$ROOT_DIR/target/platform_matrix/linux_ecs"
  local vulkan_output="$ROOT_DIR/target/platform_matrix/linux_vulkan"
  mkdir -p "$(dirname "$ecs_output")"
  local status="success"
  local message=""

  if ! "$STAGE3_BIN" build "$ROOT_DIR/examples/seen-ecs-min/main.seen" "$ecs_output" >/tmp/linux_ecs.log 2>&1; then
    status="failure"
    message="failed to build seen-ecs-min"
  elif ! "$STAGE3_BIN" run "$ROOT_DIR/examples/seen-ecs-min/main.seen" >/tmp/linux_ecs_run.log 2>&1; then
    status="failure"
    message="failed to run seen-ecs-min"
  fi

  if [[ "$status" == "success" ]]; then
    if ! "$STAGE3_BIN" build "$ROOT_DIR/examples/seen-vulkan-min/main.seen" "$vulkan_output" >/tmp/linux_vulkan.log 2>&1; then
      status="failure"
      message="failed to build seen-vulkan-min"
    fi
  fi

  cat > "$report" <<JSON
{
  "platform": "$platform",
  "status": "$status",
  "stage3": "${STAGE3_BIN}",
  "ecs_log": "$(sed 's/"/\\"/g' </tmp/linux_ecs.log 2>/dev/null)",
  "ecs_run_log": "$(sed 's/"/\\"/g' </tmp/linux_ecs_run.log 2>/dev/null)",
  "vulkan_log": "$(sed 's/"/\\"/g' </tmp/linux_vulkan.log 2>/dev/null)",
  "message": "$message"
}
JSON
}

run_placeholder() {
  local platform="$1"
  local report="$2"
  cat > "$report" <<JSON
{
  "platform": "$platform",
  "status": "pending",
  "message": "Platform harness not implemented yet",
  "stage3": "${STAGE3_BIN}",
  "notes": "Provision agent + run seen run/build for examples"
}
JSON
}

for platform in "${TARGET_PLATFORMS[@]}"; do
  report="$REPORT_DIR/${platform}.json"
  echo "[matrix] running $platform"
  case "$platform" in
    linux-*)
      run_linux "$platform" "$report"
      ;;
    windows-*|macos-*|android-*|ios-*|web-*)
      run_placeholder "$platform" "$report"
      ;;
    *)
      echo "Unknown platform $platform" >&2
      exit 1
      ;;
  esac
 done

echo "Platform reports written to $REPORT_DIR"
