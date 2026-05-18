#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")"/.. && pwd)"
COMPILER_BIN="$ROOT_DIR/compiler_seen/target/seen"
OUTPUT_ROOT="$ROOT_DIR/artifacts/riscv64-qemu-user"
TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
REQUIRE=0
RELEASE_MODE=0
RUN_ONLY=0
QEMU_BIN="${SEEN_QEMU_RISCV64:-}"
SYSROOT="${SEEN_LINUX_RISCV64_SYSROOT:-${RISCV64_SYSROOT:-}}"
SELECTED_SOURCES=()

DEFAULT_SOURCES=(
  "$ROOT_DIR/examples/hello_world/hello_english.seen"
  "$ROOT_DIR/tests/codegen/test_comptime_target_predicates.seen"
  "$ROOT_DIR/seen_std/tests/str_basic.seen"
  "$ROOT_DIR/seen_std/tests/string_buffer_basic.seen"
  "$ROOT_DIR/seen_std/tests/hash_map_basic.seen"
  "$ROOT_DIR/tests/memory/test_memory_basic_safety.seen"
)

usage() {
  cat <<'USAGE'
test_riscv64_qemu.sh - compile and run Linux RISC-V 64 Seen smoke tests under QEMU user-mode.

Usage: scripts/test_riscv64_qemu.sh [options]

Options:
  --compiler <path>     Path to Seen compiler binary
  --source <file>       Limit to one source file (repeatable)
  --output-dir <dir>    Output directory for logs and artifacts
  --qemu <path>         qemu-riscv64 or qemu-riscv64-static binary
  --sysroot <dir>       RISC-V Linux sysroot used by qemu -L and clang
  --release             Add --release to compiler invocations
  --run-only            Run existing artifacts in the output dir; do not compile
  --require             Fail instead of skipping when tools are missing
  -h, --help            Show this help message

Environment:
  SEEN_QEMU_RISCV64
  SEEN_LINUX_RISCV64_SYSROOT or RISCV64_SYSROOT
  SEEN_LINUX_RISCV64_GCC_TOOLCHAIN
  SEEN_LINUX_RISCV64_TRIPLE
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --compiler)
      COMPILER_BIN="$2"
      shift 2
      ;;
    --source)
      SELECTED_SOURCES+=("$2")
      shift 2
      ;;
    --output-dir)
      OUTPUT_ROOT="$2"
      shift 2
      ;;
    --qemu)
      QEMU_BIN="$2"
      shift 2
      ;;
    --sysroot)
      SYSROOT="$2"
      export SEEN_LINUX_RISCV64_SYSROOT="$SYSROOT"
      export RISCV64_SYSROOT="$SYSROOT"
      shift 2
      ;;
    --release)
      RELEASE_MODE=1
      shift
      ;;
    --run-only)
      RUN_ONLY=1
      shift
      ;;
    --require)
      REQUIRE=1
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

skip_or_fail() {
  local message="$1"
  echo "[riscv64-qemu] unavailable: $message" >&2
  echo "[riscv64-qemu] Arch setup: sudo pacman -Syu --needed clang llvm lld file qemu-user qemu-user-static qemu-system-riscv qemu-system-riscv-firmware riscv64-linux-gnu-binutils riscv64-linux-gnu-gcc riscv64-linux-gnu-glibc" >&2
  if [[ "$REQUIRE" -eq 1 ]]; then
    exit 1
  fi
  exit 0
}

find_qemu() {
  if [[ -n "$QEMU_BIN" ]]; then
    [[ -x "$QEMU_BIN" ]] && return 0
    skip_or_fail "configured QEMU binary is not executable: $QEMU_BIN"
  fi
  QEMU_BIN="$(command -v qemu-riscv64 || true)"
  if [[ -z "$QEMU_BIN" ]]; then
    QEMU_BIN="$(command -v qemu-riscv64-static || true)"
  fi
  [[ -n "$QEMU_BIN" ]] || skip_or_fail "qemu-riscv64 or qemu-riscv64-static not found"
}

find_sysroot() {
  if [[ -n "$SYSROOT" && -d "$SYSROOT" ]]; then
    export SEEN_LINUX_RISCV64_SYSROOT="$SYSROOT"
    export RISCV64_SYSROOT="$SYSROOT"
    return 0
  fi
  if [[ -d "$ROOT_DIR/artifacts/toolchains/linux-riscv64/usr/riscv64-linux-gnu" ]]; then
    SYSROOT="$ROOT_DIR/artifacts/toolchains/linux-riscv64/usr/riscv64-linux-gnu"
  elif [[ -d /usr/riscv64-linux-gnu ]]; then
    SYSROOT="/usr/riscv64-linux-gnu"
  elif [[ -d /usr/local/riscv64-linux-gnu ]]; then
    SYSROOT="/usr/local/riscv64-linux-gnu"
  elif [[ -d /opt/riscv64-linux-gnu ]]; then
    SYSROOT="/opt/riscv64-linux-gnu"
  else
    skip_or_fail "RISC-V sysroot not found; set SEEN_LINUX_RISCV64_SYSROOT or run scripts/setup_linux_riscv64_sysroot.sh"
  fi
  export SEEN_LINUX_RISCV64_SYSROOT="$SYSROOT"
  export RISCV64_SYSROOT="$SYSROOT"
}

case_name_for_source() {
  local source_file="$1"
  local case_name
  case_name="$(basename "$source_file")"
  case_name="${case_name%.seen}"
  case_name="${case_name//[^A-Za-z0-9._-]/_}"
  printf '%s' "$case_name"
}

artifact_is_riscv64() {
  local artifact="$1"
  local description=""
  if command -v llvm-objdump >/dev/null 2>&1; then
    description="$(llvm-objdump -f "$artifact" 2>/dev/null || true)"
    [[ "$description" == *"elf64-littleriscv"* || "$description" == *"riscv64"* ]] && return 0
  fi
  if command -v file >/dev/null 2>&1; then
    description="$(file -b "$artifact" 2>/dev/null || true)"
    [[ "$description" == *"ELF"* && ( "$description" == *"RISC-V"* || "$description" == *"riscv"* ) ]] && return 0
  fi
  return 1
}

find_qemu
find_sysroot
command -v clang >/dev/null 2>&1 || skip_or_fail "clang not found"
command -v llc >/dev/null 2>&1 || skip_or_fail "llc not found"
command -v lld >/dev/null 2>&1 || command -v ld.lld >/dev/null 2>&1 || skip_or_fail "LLD linker not found"

if [[ "$RUN_ONLY" -eq 0 && ! -x "$COMPILER_BIN" ]]; then
  skip_or_fail "compiler binary not found or not executable: $COMPILER_BIN"
fi

SOURCES=()
if [[ ${#SELECTED_SOURCES[@]} -gt 0 ]]; then
  SOURCES=("${SELECTED_SOURCES[@]}")
else
  SOURCES=("${DEFAULT_SOURCES[@]}")
fi

RUN_DIR="$OUTPUT_ROOT/$TIMESTAMP"
mkdir -p "$RUN_DIR"
SUMMARY="$RUN_DIR/summary.tsv"
printf 'case\tsource\tcompile_status\trun_status\tartifact\tnote\n' > "$SUMMARY"

echo "[riscv64-qemu] qemu: $QEMU_BIN"
echo "[riscv64-qemu] sysroot: $SYSROOT"
echo "[riscv64-qemu] output: $RUN_DIR"

has_failures=0
for source_file in "${SOURCES[@]}"; do
  case_name="$(case_name_for_source "$source_file")"
  case_dir="$RUN_DIR/$case_name"
  artifact="$case_dir/$case_name"
  compile_log="$case_dir/compile.log"
  run_log="$case_dir/run.log"
  note=""
  compile_status="success"
  run_status="success"
  mkdir -p "$case_dir"

  if [[ ! -f "$source_file" ]]; then
    compile_status="failure"
    run_status="skipped"
    note="source file not found"
    has_failures=1
  elif [[ "$RUN_ONLY" -eq 0 ]]; then
    command=("$COMPILER_BIN" compile "$source_file" "$artifact" --backend llvm --target=linux-riscv64)
    if [[ "$RELEASE_MODE" -eq 1 ]]; then
      command+=(--release)
    fi
    echo "[riscv64-qemu] compiling $case_name"
    if command -v timeout >/dev/null 2>&1; then
      timeout 900 "${command[@]}" >"$compile_log" 2>&1 || compile_status="failure"
    else
      "${command[@]}" >"$compile_log" 2>&1 || compile_status="failure"
    fi
    if [[ "$compile_status" == "success" && ! -x "$artifact" ]]; then
      compile_status="failure"
      note="artifact was not produced"
    fi
    if [[ "$compile_status" == "success" ]] && ! artifact_is_riscv64 "$artifact"; then
      compile_status="failure"
      note="artifact is not a RISC-V ELF"
    fi
    if [[ "$compile_status" != "success" ]]; then
      run_status="skipped"
      [[ -n "$note" ]] || note="$(tail -n 1 "$compile_log" 2>/dev/null || echo 'compile failed')"
      has_failures=1
    fi
  elif [[ ! -x "$artifact" ]]; then
    compile_status="skipped"
    run_status="failure"
    note="run-only artifact missing: $artifact"
    has_failures=1
  fi

  if [[ "$compile_status" == "success" ]]; then
    echo "[riscv64-qemu] running $case_name"
    if command -v timeout >/dev/null 2>&1; then
      timeout 120 "$QEMU_BIN" -L "$SYSROOT" "$artifact" >"$run_log" 2>&1 || run_status="failure"
    else
      "$QEMU_BIN" -L "$SYSROOT" "$artifact" >"$run_log" 2>&1 || run_status="failure"
    fi
    if [[ "$run_status" != "success" ]]; then
      [[ -n "$note" ]] || note="$(tail -n 1 "$run_log" 2>/dev/null || echo 'qemu run failed')"
      has_failures=1
    fi
  fi

  printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$case_name" \
    "${source_file#$ROOT_DIR/}" \
    "$compile_status" \
    "$run_status" \
    "$artifact" \
    "$note" >> "$SUMMARY"
done

echo "[riscv64-qemu] summary: $SUMMARY"
if [[ "$has_failures" -ne 0 ]]; then
  exit 1
fi
