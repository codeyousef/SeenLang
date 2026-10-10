#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED="$ROOT_DIR/scripts/run_capped_regression.sh"
ASAN_RUNNER="$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh"
SCOPE=seen-sha256-file-ownership

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi

COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
SOURCE="$ROOT_DIR/tests/fixtures/fel_2378_sha256_file_ownership.seen"
INPUT="$ROOT_DIR/tests/fixtures/fel_2378_sha256_file_input.txt"
WORK="$(mktemp -d "$SEEN_ARTIFACT_ROOT/sha256-file-ownership.XXXXXX")"
mkdir -p -- "$WORK/bin" "$WORK/logs"

if ! ulimit -S -s 8192 2>/dev/null || [ "$(ulimit -S -s)" != 8192 ]; then
    echo 'FAIL: FEL-2378 requires a verified 8192 KiB stack' >&2
    exit 126
fi

compile_and_run() {
    local profile=$1
    shift
    local binary="$WORK/bin/$profile"
    local compile_log="$WORK/logs/$profile.compile.log"
    local run_log="$WORK/logs/$profile.run.log"
    local status=0

    timeout --foreground --kill-after=10s 900s \
        bash "$ATTESTED_SEEN" "$COMPILER" compile "$SOURCE" "$binary" \
            --no-cache --jobs 1 --opt-jobs 1 "$@" \
            >"$compile_log" 2>&1 || status=$?
    if [ "$status" -ne 0 ]; then
        tail -c 32768 -- "$compile_log" >&2 || true
        return "$status"
    fi

    if [ "$profile" = address ]; then
        timeout --foreground --kill-after=5s 120s \
            bash "$ASAN_RUNNER" --target-root "$WORK/bin" \
                --compile-log "$compile_log" -- "$binary" "$INPUT" \
                "$WORK/missing-file" >"$run_log" 2>&1 || status=$?
    else
        timeout --foreground --kill-after=5s 120s \
            env UBSAN_OPTIONS=abort_on_error=1:halt_on_error=1 \
                "$binary" "$INPUT" "$WORK/missing-file" \
                >"$run_log" 2>&1 || status=$?
    fi
    if [ "$status" -ne 0 ] || \
        grep -Eiq 'ERROR: (AddressSanitizer|LeakSanitizer)|SUMMARY: (AddressSanitizer|UndefinedBehaviorSanitizer)|runtime error:' "$run_log"; then
        tail -c 32768 -- "$run_log" >&2 || true
        return 1
    fi
    grep -Fxq 'PASS: FEL-2378 SHA-256 file allocator baseline' "$run_log"
}

compile_and_run fast --fast
compile_and_run release --release --lto=thin --target-cpu=x86-64
compile_and_run undefined --fast --sanitize undefined
compile_and_run address --fast --sanitize address

echo 'PASS: FEL-2378 fast, release/ThinLTO, UBSan, and ASan ownership'
