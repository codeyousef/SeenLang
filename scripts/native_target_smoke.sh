#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")"/.. && pwd)"
COMPILER_BIN="$ROOT_DIR/compiler_seen/target/seen"
DEFAULT_SOURCE_FILE="$ROOT_DIR/examples/hello_world/hello_english.seen"
SOURCE_OVERRIDE=""
OUTPUT_ROOT="$ROOT_DIR/artifacts/native-target-smoke"
TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
RELEASE_MODE=0
DEFAULT_TARGETS=(
  "linux-x86_64"
  "linux-arm64"
  "windows-x86_64"
  "macos-x86_64"
  "macos-arm64"
  "ios-arm64"
  "android-arm64"
)

usage() {
  cat <<'USAGE'
native_target_smoke.sh - compile a small Seen program across native targets.

Usage: scripts/native_target_smoke.sh [options]

Options:
  --compiler <path>     Path to Seen compiler binary
  --source <file>       Override the default target-specific source list with one file
  --output-dir <dir>    Output directory for reports and artifacts
  --target <name>       Limit run to a single target (repeatable)
  --release             Add --release to compiler invocations
  -h, --help            Show this help message

Statuses:
  success      Target build completed and produced an artifact.
  unavailable  Required host-side SDK/toolchain probe failed.
  failure      Build was attempted and failed.
USAGE
}

SELECTED_TARGETS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --compiler)
      COMPILER_BIN="$2"
      shift 2
      ;;
    --source)
      SOURCE_OVERRIDE="$2"
      shift 2
      ;;
    --output-dir)
      OUTPUT_ROOT="$2"
      shift 2
      ;;
    --target)
      SELECTED_TARGETS+=("$2")
      shift 2
      ;;
    --release)
      RELEASE_MODE=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ ! -x "$COMPILER_BIN" ]]; then
  echo "Compiler binary not found or not executable: $COMPILER_BIN" >&2
  exit 1
fi

if [[ -n "$SOURCE_OVERRIDE" && ! -f "$SOURCE_OVERRIDE" ]]; then
  echo "Source file not found: $SOURCE_OVERRIDE" >&2
  exit 1
fi

if [[ -z "$SOURCE_OVERRIDE" && ! -f "$DEFAULT_SOURCE_FILE" ]]; then
  echo "Default source file not found: $DEFAULT_SOURCE_FILE" >&2
  exit 1
fi

TARGETS=()
if [[ ${#SELECTED_TARGETS[@]} -gt 0 ]]; then
  TARGETS=("${SELECTED_TARGETS[@]}")
else
  TARGETS=("${DEFAULT_TARGETS[@]}")
fi

CLI_HELP="$($COMPILER_BIN 2>&1 || true)"
CLI_SUBCOMMAND="compile"
if grep -q 'seen build <' <<< "$CLI_HELP"; then
  CLI_SUBCOMMAND="build"
fi

RUN_DIR="$OUTPUT_ROOT/$TIMESTAMP"
mkdir -p "$RUN_DIR"
SUMMARY_TSV="$RUN_DIR/summary.tsv"
printf 'target\tstatus\tartifact\tnote\n' > "$SUMMARY_TSV"

has_failures=0

artifact_name_for_target() {
  local target="$1"
  case "$target" in
    windows-*)
      printf 'hello_smoke.exe'
      ;;
    android-*)
      printf 'libhello_smoke.so'
      ;;
    *)
      printf 'hello_smoke'
      ;;
  esac
}

case_name_for_source() {
  local source_file="$1"
  local case_name
  case_name="$(basename "$source_file")"
  case_name="${case_name%.seen}"
  case_name="${case_name//[^A-Za-z0-9._-]/_}"
  printf '%s' "$case_name"
}

sources_for_target() {
  local target="$1"
  if [[ -n "$SOURCE_OVERRIDE" ]]; then
    printf '%s\n' "$SOURCE_OVERRIDE"
    return
  fi

  printf '%s\n' "$DEFAULT_SOURCE_FILE"
  case "$target" in
    windows-*)
      printf '%s\n' "$ROOT_DIR/tests/codegen/test_game_engine_features.seen"
      printf '%s\n' "$ROOT_DIR/seen_std/tests/hash_map_basic.seen"
      printf '%s\n' "$ROOT_DIR/seen_std/tests/string_hash_map_basic.seen"
      printf '%s\n' "$ROOT_DIR/seen_std/tests/str_basic.seen"
      ;;
    android-*)
      printf '%s\n' "$ROOT_DIR/tests/gpu/test_compute_basic.seen"
      printf '%s\n' "$ROOT_DIR/seen_std/tests/hash_map_basic.seen"
      printf '%s\n' "$ROOT_DIR/seen_std/tests/string_hash_map_basic.seen"
      printf '%s\n' "$ROOT_DIR/seen_std/tests/str_basic.seen"
      ;;
  esac
}

artifact_name_for_case() {
  local target="$1"
  local case_name="$2"
  local case_index="$3"

  if [[ "$case_index" -eq 0 ]]; then
    artifact_name_for_target "$target"
    return
  fi

  case "$target" in
    windows-*)
      printf '%s.exe' "$case_name"
      ;;
    android-*)
      printf 'lib%s.so' "$case_name"
      ;;
    *)
      printf '%s' "$case_name"
      ;;
  esac
}

artifact_matches_target() {
  local target="$1"
  local description="$2"
  case "$target" in
    linux-x86_64)
      [[ "$description" == *"ELF"* && "$description" == *"x86-64"* ]]
      ;;
    linux-arm64)
      [[ "$description" == *"ELF"* && ( "$description" == *"ARM aarch64"* || "$description" == *"ARM64"* ) ]]
      ;;
    windows-*)
      [[ "$description" == *"PE32"* || "$description" == *"MS Windows"* ]]
      ;;
    macos-x86_64)
      [[ "$description" == *"Mach-O"* && "$description" == *"x86_64"* ]]
      ;;
    macos-arm64)
      [[ "$description" == *"Mach-O"* && ( "$description" == *"arm64"* || "$description" == *"aarch64"* ) ]]
      ;;
    ios-arm64)
      [[ "$description" == *"Mach-O"* && ( "$description" == *"arm64"* || "$description" == *"aarch64"* ) ]]
      ;;
    android-*)
      [[ "$description" == *"ELF"* && ( "$description" == *"ARM aarch64"* || "$description" == *"ARM64"* ) && ( "$description" == *"shared object"* || "$description" == *"dynamically linked"* ) ]]
      ;;
    *)
      return 0
      ;;
  esac
}

preflight_target() {
  local target="$1"
  case "$target" in
    linux-arm64)
      if [[ "$(uname -m)" == "aarch64" || "$(uname -m)" == "arm64" ]]; then
        return 0
      fi
      if [[ -n "${SEEN_LINUX_ARM64_SYSROOT:-}" && -d "${SEEN_LINUX_ARM64_SYSROOT}" ]]; then
        return 0
      fi
      if [[ -d /usr/aarch64-linux-gnu ]]; then
        return 0
      fi
      echo "Linux ARM64 cross-compilation requires SEEN_LINUX_ARM64_SYSROOT or /usr/aarch64-linux-gnu"
      return 1
      ;;
    android-*)
      if [[ -n "${ANDROID_NDK_HOME:-}" && -d "${ANDROID_NDK_HOME:-}" ]]; then
        return 0
      fi
      if [[ -n "${ANDROID_NDK_ROOT:-}" && -d "${ANDROID_NDK_ROOT:-}" ]]; then
        return 0
      fi
      echo "ANDROID_NDK_HOME or ANDROID_NDK_ROOT is required"
      return 1
      ;;
    macos-*|ios-*)
      if command -v xcrun >/dev/null 2>&1; then
        return 0
      fi
      echo "xcrun is required for Apple target smoke tests"
      return 1
      ;;
    *)
      return 0
      ;;
  esac
}

escape_field() {
  local value="$1"
  value="${value//$'\t'/ }"
  value="${value//$'\n'/ }"
  printf '%s' "$value"
}

run_build() {
  local source_file="$1"
  local target="$2"
  local artifact="$3"
  local log_file="$4"
  local -a command
  if [[ "$CLI_SUBCOMMAND" == "build" ]]; then
    command=("$COMPILER_BIN" build "$source_file" --backend llvm --target "$target" --output "$artifact")
  else
    command=("$COMPILER_BIN" compile "$source_file" "$artifact" --backend llvm "--target=$target")
  fi
  if [[ "$RELEASE_MODE" -eq 1 ]]; then
    command+=(--release)
  fi
  if command -v timeout >/dev/null 2>&1; then
    timeout 600 "${command[@]}" >"$log_file" 2>&1
  else
    "${command[@]}" >"$log_file" 2>&1
  fi
}

for target in "${TARGETS[@]}"; do
  target_dir="$RUN_DIR/$target"
  mkdir -p "$target_dir"
  log_file="$target_dir/build.log"
  case_results_file="$target_dir/case-results.tsv"
  artifact_path=""
  note=""
  status="success"

  printf 'case\tsource\tstatus\tartifact\tnote\n' > "$case_results_file"
  : > "$log_file"

  if ! preflight_msg="$(preflight_target "$target" 2>&1)"; then
    status="unavailable"
    note="$preflight_msg"
  else
    mapfile -t target_sources < <(sources_for_target "$target")
    case_index=0
    for source_file in "${target_sources[@]}"; do
      case_name="$(case_name_for_source "$source_file")"
      case_dir="$target_dir/$case_name"
      case_log="$case_dir/build.log"
      case_artifact="$case_dir/$(artifact_name_for_case "$target" "$case_name" "$case_index")"
      case_status="success"
      case_note=""

      mkdir -p "$case_dir"
      if [[ -z "$artifact_path" ]]; then
        artifact_path="$case_artifact"
      fi

      if [[ "$RELEASE_MODE" -eq 1 ]]; then
        echo "[native-smoke] building $target:$case_name from ${source_file#$ROOT_DIR/} [release]" >> "$log_file"
      else
        echo "[native-smoke] building $target:$case_name from ${source_file#$ROOT_DIR/}" >> "$log_file"
      fi

      if [[ ! -f "$source_file" ]]; then
        case_status="failure"
        case_note="source file not found: $source_file"
        printf '%s\n' "$case_note" > "$case_log"
        has_failures=1
      elif run_build "$source_file" "$target" "$case_artifact" "$case_log"; then
        if [[ ! -e "$case_artifact" ]]; then
          case_status="failure"
          case_note="build command succeeded but artifact was not produced"
          has_failures=1
        else
          if command -v file >/dev/null 2>&1; then
            case_note="$(file -b "$case_artifact" 2>/dev/null || true)"
          else
            case_note="artifact created"
          fi
          if ! artifact_matches_target "$target" "$case_note"; then
            case_status="failure"
            case_note="artifact format does not match target: $case_note"
            has_failures=1
          fi
        fi
      else
        case_status="failure"
        case_note="$(tail -n 1 "$case_log" 2>/dev/null || echo 'build failed')"
        has_failures=1
      fi

      {
        echo "=== $case_name (${source_file#$ROOT_DIR/}) ==="
        cat "$case_log"
        echo
      } >> "$log_file"

      if [[ "$case_status" == "failure" ]]; then
        status="failure"
      fi

      case_summary="$case_name=$case_status"
      if [[ -n "$case_note" ]]; then
        case_summary="$case_summary ($case_note)"
      fi
      if [[ -n "$note" ]]; then
        note="$note; $case_summary"
      else
        note="$case_summary"
      fi

      case_output_note="$case_note"
      if [[ "$RELEASE_MODE" -eq 1 ]]; then
        if [[ -n "$case_output_note" ]]; then
          case_output_note="mode=release; $case_output_note"
        else
          case_output_note="mode=release"
        fi
      fi

      printf '%s\t%s\t%s\t%s\t%s\n' \
        "$case_name" \
        "$(escape_field "${source_file#$ROOT_DIR/}")" \
        "$case_status" \
        "$(escape_field "$case_artifact")" \
        "$(escape_field "$case_output_note")" >> "$case_results_file"

      case_index=$((case_index + 1))
    done
  fi

  if [[ "$RELEASE_MODE" -eq 1 ]]; then
    if [[ -n "$note" ]]; then
      note="mode=release; $note"
    else
      note="mode=release"
    fi
  fi

  printf '%s\t%s\t%s\t%s\n' \
    "$target" \
    "$status" \
    "$(escape_field "$artifact_path")" \
    "$(escape_field "$note")" >> "$SUMMARY_TSV"
done

echo "Native target smoke summary: $SUMMARY_TSV"
if [[ "$has_failures" -ne 0 ]]; then
  exit 1
fi
