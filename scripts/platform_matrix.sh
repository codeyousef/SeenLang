#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")"/.. && pwd)"
OUTPUT_DIR="$ROOT_DIR/artifacts/platform-matrix"
STAGE3_BIN=""
TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
SMOKE_HARNESS="$ROOT_DIR/scripts/native_target_smoke.sh"
RUN_LINUX_RUNTIME=0
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
  --with-runtime       Run Linux example programs in addition to compile smoke
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
    --with-runtime)
      RUN_LINUX_RUNTIME=1
      shift
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

if [[ ! -f "$SMOKE_HARNESS" ]]; then
  echo "native smoke harness not found: $SMOKE_HARNESS" >&2
  exit 1
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
  local smoke_dir="$REPORT_DIR/native-smoke-$platform"
  local smoke_status="failure"
  local smoke_artifact=""
  local smoke_note=""
  local smoke_log="$smoke_dir/$platform/build.log"
  local runtime_status="success"
  local runtime_message=""

  mkdir -p "$smoke_dir"

  if bash "$SMOKE_HARNESS" --compiler "$STAGE3_BIN" --output-dir "$smoke_dir" --target "$platform" >/tmp/platform_matrix_linux_smoke.log 2>&1; then
    :
  fi
  parse_smoke_summary "$smoke_dir" "$platform" smoke_status smoke_artifact smoke_note

  if [[ "$RUN_LINUX_RUNTIME" -eq 1 ]]; then
    if ! run_with_timeout_capture /tmp/linux_ecs_run.log "$STAGE3_BIN" run "$ROOT_DIR/examples/seen-ecs-min/main.seen"; then
      runtime_status="failure"
      runtime_message="failed to run seen-ecs-min"
    fi

    if [[ "$runtime_status" == "success" ]]; then
      if ! run_with_timeout_capture /tmp/linux_vulkan.log "$STAGE3_BIN" run "$ROOT_DIR/examples/seen-vulkan-min/main.seen"; then
        runtime_status="failure"
        runtime_message="failed to run seen-vulkan-min"
      fi
    fi
  else
    runtime_status="skipped"
    runtime_message="Linux runtime examples skipped; pass --with-runtime to execute them"
  fi

  cat > "$report" <<JSON
{
  "platform": "$platform",
  "status": "$runtime_status",
  "stage3": "${STAGE3_BIN}",
  "smoke_status": "$smoke_status",
  "smoke_artifact": "$(json_escape "$smoke_artifact")",
  "smoke_note": "$(json_escape "$smoke_note")",
  "smoke_log": "$(json_escape "$smoke_log")",
  "ecs_log": "$(json_escape_file /tmp/linux_ecs.log)",
  "ecs_run_log": "$(json_escape_file /tmp/linux_ecs_run.log)",
  "vulkan_log": "$(json_escape_file /tmp/linux_vulkan.log)",
  "message": "$(json_escape "$runtime_message")"
}
JSON
}

json_escape() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//"/\\"}"
  value="${value//$'\n'/ }"
  value="${value//$'\r'/ }"
  printf '%s' "$value"
}

json_escape_file() {
  local file_path="$1"
  if [[ ! -f "$file_path" ]]; then
    printf ''
    return
  fi
  sed 's/\\/\\\\/g; s/"/\\"/g' < "$file_path" | tr '\n' ' '
}

parse_smoke_summary() {
  local smoke_root="$1"
  local platform="$2"
  local -n out_status_ref="$3"
  local -n out_artifact_ref="$4"
  local -n out_note_ref="$5"
  local summary_file
  summary_file="$(find "$smoke_root" -name summary.tsv -print | head -n 1)"
  if [[ -z "$summary_file" || ! -f "$summary_file" ]]; then
    out_status_ref="failure"
    out_artifact_ref=""
    out_note_ref="smoke summary not generated"
    return
  fi
  local row
  row="$(awk -F '\t' -v target="$platform" 'NR > 1 && $1 == target { print; exit }' "$summary_file")"
  if [[ -z "$row" ]]; then
    out_status_ref="failure"
    out_artifact_ref=""
    out_note_ref="target missing from smoke summary"
    return
  fi
  IFS=$'\t' read -r _target out_status_ref out_artifact_ref out_note_ref <<< "$row"
}

run_with_timeout_capture() {
  local log_file="$1"
  shift
  if command -v timeout >/dev/null 2>&1; then
    timeout 180 "$@" >"$log_file" 2>&1
  else
    "$@" >"$log_file" 2>&1
  fi
}

run_native_smoke_report() {
  local platform="$1"
  local report="$2"
  local smoke_dir="$REPORT_DIR/native-smoke-$platform"
  local status="failure"
  local artifact=""
  local note=""
  local smoke_log="$smoke_dir/$platform/build.log"

  mkdir -p "$smoke_dir"
  if bash "$SMOKE_HARNESS" --compiler "$STAGE3_BIN" --output-dir "$smoke_dir" --target "$platform" >/tmp/platform_matrix_${platform}.log 2>&1; then
    :
  fi
  parse_smoke_summary "$smoke_dir" "$platform" status artifact note

  cat > "$report" <<JSON
{
  "platform": "$platform",
  "status": "$status",
  "stage3": "${STAGE3_BIN}",
  "artifact": "$(json_escape "$artifact")",
  "log": "$(json_escape "$smoke_log")",
  "message": "$(json_escape "$note")"
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
    windows-*|macos-*|android-*|ios-*)
      run_native_smoke_report "$platform" "$report"
      ;;
    web-*)
      cat > "$report" <<JSON
{
  "platform": "$platform",
  "status": "pending",
  "message": "Web harness not implemented yet",
  "stage3": "${STAGE3_BIN}",
  "notes": "Native rollout intentionally excludes WASM until native validation is stable"
}
JSON
      ;;
    *)
      echo "Unknown platform $platform" >&2
      exit 1
      ;;
  esac
 done

echo "Platform reports written to $REPORT_DIR"
