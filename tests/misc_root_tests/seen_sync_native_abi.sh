#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
ASAN_RUNNER="$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh"
SCOPE=seen-sync-native-abi

if [ "${SEEN_CAPPED_PLATFORM_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" --platform "$SCOPE" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-platform-active "$SCOPE"
[ "${SEEN_LOW_MEMORY:-0}" = 1 ] && [ "${SEEN_JOBS:-0}" = 1 ] &&
    [ "${SEEN_OPT_JOBS:-0}" = 1 ] || {
    echo "FAIL: sync ABI contract requires serial low-memory settings" >&2
    exit 126
}

WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/sync-abi.XXXXXX")"
cleanup() {
    local status=$?
    if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$WORK_DIR" in
            "$SEEN_ARTIFACT_ROOT"/sync-abi.*)
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    rm -rf -- "$WORK_DIR"
                ;;
            *) status=1 ;;
        esac
    else
        echo "Preserved sync ABI artifacts: $WORK_DIR" >&2
    fi
    exit "$status"
}
trap cleanup EXIT

compile_probe() {
    local label=$1
    shift
    local compile_log="$WORK_DIR/$label.compile.log"
    (
        set -x
        clang -std=c11 -O2 -g -Wall -Wextra -Werror -pthread \
            -I "$ROOT_DIR/seen_runtime" "$@" \
            "$ROOT_DIR/tests/compiler_regressions/sync_abi_contract.c" \
            "$ROOT_DIR/seen_runtime/sync_abi.c" -o "$WORK_DIR/$label"
    ) >"$compile_log" 2>&1
    if [ "$label" = address ]; then
        bash "$ASAN_RUNNER" --target-root "$WORK_DIR" \
            --compile-log "$compile_log" -- "$WORK_DIR/$label"
    else
        timeout --foreground --kill-after=5s 60s "$WORK_DIR/$label"
    fi
}

compile_probe native
compile_probe undefined -fsanitize=undefined
compile_probe address -fsanitize=address

if command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
    x86_64-w64-mingw32-gcc -std=c11 -O2 -Wall -Wextra -Werror \
        -I "$ROOT_DIR/seen_runtime" -c "$ROOT_DIR/seen_runtime/sync_abi.c" \
        -o "$WORK_DIR/sync-abi-windows.o"
fi

echo "PASS: SYNC-001A-H native ABI contract"
