#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
ASAN_RUNNER="$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh"
SCOPE=seen-reactor-contract
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
[ "${SEEN_LOW_MEMORY:-0}" = 1 ] && [ "${SEEN_JOBS:-0}" = 1 ] &&
    [ "${SEEN_OPT_JOBS:-0}" = 1 ] || {
    echo "FAIL: reactor contract requires serial low-memory settings" >&2
    exit 126
}

WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/reactor-contract.XXXXXX")"
cleanup() {
    local status=$?
    if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$WORK_DIR" in
            "$SEEN_ARTIFACT_ROOT"/reactor-contract.*)
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    rm -rf -- "$WORK_DIR"
                ;;
            *) status=1 ;;
        esac
    else
        echo "Preserved reactor artifacts: $WORK_DIR" >&2
    fi
    exit "$status"
}
trap cleanup EXIT

compile_c_probe() {
    local label=$1
    shift
    local compile_log="$WORK_DIR/$label.compile.log"
    (
        set -x
        clang -std=c11 -O2 -g -Wall -Wextra -Werror -pthread \
            -I "$ROOT_DIR/seen_runtime" "$@" \
            "$ROOT_DIR/tests/compiler_regressions/reactor_abi_contract.c" \
            "$ROOT_DIR/seen_runtime/reactor_abi.c" -o "$WORK_DIR/$label"
    ) >"$compile_log" 2>&1
    if [ "$label" = address ]; then
        bash "$ASAN_RUNNER" --target-root "$WORK_DIR" \
            --compile-log "$compile_log" -- "$WORK_DIR/$label"
    else
        timeout --foreground --kill-after=5s 60s "$WORK_DIR/$label"
    fi
}

compile_c_probe native
compile_c_probe undefined -fsanitize=undefined
compile_c_probe address -fsanitize=address

if command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
    x86_64-w64-mingw32-gcc -std=c11 -O2 -Wall -Wextra -Werror \
        -I "$ROOT_DIR/seen_runtime" -c \
        "$ROOT_DIR/seen_runtime/reactor_abi.c" \
        -o "$WORK_DIR/reactor-abi-windows.o"
fi

python3 "$ROOT_DIR/scripts/check_native_boundaries.py" \
    "$ROOT_DIR/docs/architecture/native-boundaries.json"

echo "PASS: REACTOR-001A-H contract"
