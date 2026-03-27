#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")"/.. && pwd)"
COMPILER_BIN="$ROOT_DIR/compiler_seen/target/seen"
SOURCE_FILE="$ROOT_DIR/examples/hello_world/hello_english.seen"
OUTPUT_ROOT="$ROOT_DIR/artifacts/native-target-smoke"
TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
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
  --source <file>       Seen source file to compile
  --output-dir <dir>    Output directory for reports and artifacts
  --target <name>       Limit run to a single target (repeatable)
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
      SOURCE_FILE="$2"
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

if [[ ! -f "$SOURCE_FILE" ]]; then
  echo "Source file not found: $SOURCE_FILE" >&2
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
  local target="$1"
  local artifact="$2"
  local log_file="$3"
  local -a command
  if [[ "$CLI_SUBCOMMAND" == "build" ]]; then
    command=("$COMPILER_BIN" build "$SOURCE_FILE" --backend llvm --target "$target" --output "$artifact")
  else
    command=("$COMPILER_BIN" compile "$SOURCE_FILE" "$artifact" --backend llvm "--target=$target")
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
  artifact_path="$target_dir/$(artifact_name_for_target "$target")"
  log_file="$target_dir/build.log"
  note=""
  status="success"

  if ! preflight_msg="$(preflight_target "$target" 2>&1)"; then
    status="unavailable"
    note="$preflight_msg"
    : > "$log_file"
  else
    echo "[native-smoke] building $target"
    if run_build "$target" "$artifact_path" "$log_file"; then
      if [[ ! -e "$artifact_path" ]]; then
        status="failure"
        note="build command succeeded but artifact was not produced"
      else
        if command -v file >/dev/null 2>&1; then
          note="$(file -b "$artifact_path" 2>/dev/null || true)"
        else
          note="artifact created"
        fi
        if ! artifact_matches_target "$target" "$note"; then
          status="failure"
          note="artifact format does not match target: $note"
          has_failures=1
        fi
      fi
    else
      status="failure"
      note="$(tail -n 1 "$log_file" 2>/dev/null || echo 'build failed')"
      has_failures=1
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
