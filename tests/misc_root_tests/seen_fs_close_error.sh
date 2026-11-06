#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-fs-close-error
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
TMP_DIR=$(mktemp -d "$SEEN_ARTIFACT_ROOT/fs-close-error.XXXXXX")
source "$ROOT_DIR/scripts/report_fixture_failure_logs.sh"
cleanup() {
    local status=$?
    trap - EXIT
    seen_report_fixture_failure_logs "$TMP_DIR"
    exit "$status"
}
trap cleanup EXIT
for profile in fast release undefined address; do
    flags=(-O1 -g -fno-omit-frame-pointer)
    case "$profile" in
        release) flags=(-O3 -flto=thin -fuse-ld=lld -Wl,--thinlto-jobs=1 -Wl,--threads=1) ;;
        undefined|address) flags+=("-fsanitize=$profile") ;;
    esac
    binary="$TMP_DIR/close-$profile"
    log="$TMP_DIR/00-active.log"
    timeout --foreground --kill-after=5s 120s bash -c 'set -x; clang "$@"' -- \
        -std=c11 "${flags[@]}" -I "$ROOT_DIR/seen_runtime" \
        "$ROOT_DIR/tests/misc_root_tests/seen_fs_close_error_contract.c" \
        "$ROOT_DIR/seen_runtime/seen_runtime.c" -pthread -ldl -lm -Wl,--wrap=close -o "$binary" > "$log" 2>&1
    cp -- "$log" "$TMP_DIR/close-$profile.compile.log"
    log="$TMP_DIR/close-$profile.compile.log"
    if [ "$profile" = address ]; then
        timeout --foreground --kill-after=5s 60s bash "$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh" \
            --target-root "$TMP_DIR" --compile-log "$log" -- "$binary" "$TMP_DIR/file-$profile" >> "$TMP_DIR/00-active.log" 2>&1
    else
        UBSAN_OPTIONS=abort_on_error=1:halt_on_error=1 \
            timeout --foreground --kill-after=5s 60s "$binary" "$TMP_DIR/file-$profile" >> "$TMP_DIR/00-active.log" 2>&1
    fi
    cp -- "$TMP_DIR/00-active.log" "$TMP_DIR/close-$profile.log"
done
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
FIXTURE_ARTIFACT_PREFIX=fs-close-error
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
driver="$TMP_DIR/close-error-driver.so"
timeout --foreground --kill-after=5s 60s clang -std=c11 -O1 -g -fPIC -shared -Wall -Wextra -Werror \
    "$ROOT_DIR/tests/misc_root_tests/seen_fs_close_error_driver.c" -o "$driver"
for profile in fast release undefined address; do
    compile_fixture "$profile" seen-close "$ROOT_DIR/seen_std/tests/fs/fs_close_error_contract.seen"
    file="$TMP_DIR/seen-file-$profile"
    if [ "$profile" = address ]; then
        SEEN_CLOSE_ERROR_PATH="$file" FIXTURE_ASAN_PRELOAD_LIBRARY="$driver" \
            run_fixture "$profile" seen-close "$file"
    else
        cp -- "$TMP_DIR/seen-close-$profile.compile.log" "$TMP_DIR/00-active.log"
        UBSAN_OPTIONS=abort_on_error=1:halt_on_error=1 \
            timeout --foreground --kill-after=5s 60s env \
            SEEN_CLOSE_ERROR_PATH="$file" LD_PRELOAD="$driver" \
            "$TMP_DIR/seen-close-$profile" "$file" >> "$TMP_DIR/00-active.log" 2>&1
        cp -- "$TMP_DIR/00-active.log" "$TMP_DIR/seen-close-$profile.log"
    fi
done
echo 'PASS: failed close retires ownership once without closing a reused descriptor'
