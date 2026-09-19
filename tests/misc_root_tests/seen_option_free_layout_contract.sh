#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
ASAN_RUNNER="$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh"
SCOPE=seen-option-free-layout
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
export SEEN_COMPILER_SOURCE_ROOT="$ROOT_DIR"
FIXTURES="$ROOT_DIR/tests/fixtures/option-free-layout"
WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/option-free-layout.XXXXXX")"

cleanup() {
    local status=$?
    case "$WORK_DIR" in
        "$SEEN_ARTIFACT_ROOT"/option-free-layout.*)
            if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    rm -rf -- "$WORK_DIR"
            else
                echo "Preserved Option layout artifacts: $WORK_DIR" >&2
            fi
            ;;
        *) status=1 ;;
    esac
    exit "$status"
}
trap cleanup EXIT

run_compiler() {
    bash "$ATTESTED_SEEN" "$COMPILER" "$@"
}

compile_and_run() {
    local fixture=$1
    local profile=$2
    local source="$FIXTURES/$fixture.seen"
    local binary="$WORK_DIR/$fixture-$profile"
    local ir_dir="$WORK_DIR/$fixture-$profile-ir"
    local log="$WORK_DIR/$fixture-$profile.log"
    local flags=(--no-cache --frozen --target-cpu=x86-64 --jobs 1
        --opt-jobs 1 --emit-module-ir-dir "$ir_dir")
    if [ "$profile" = release ]; then
        flags+=(--release --lto=thin)
    else
        flags+=(--fast)
    fi
    timeout --foreground --kill-after=10s 900s \
        bash "$ATTESTED_SEEN" "$COMPILER" check "$source" \
        >"$log" 2>&1
    timeout --foreground --kill-after=10s 900s \
        bash "$ATTESTED_SEEN" "$COMPILER" compile "$source" "$binary" \
        "${flags[@]}" \
        >>"$log" 2>&1
    timeout --foreground --kill-after=5s 120s "$binary" >>"$log" 2>&1
}

for profile in fast release; do
    compile_and_run same_module "$profile"
    compile_and_run cross_module "$profile"
    compile_and_run indexed_json "$profile"
    grep -Fxq 'PASS: same-module Option heap-box layout' \
        "$WORK_DIR/same_module-$profile.log"
    grep -Fxq 'PASS: cross-module Option heap-box layout' \
        "$WORK_DIR/cross_module-$profile.log"
    grep -Fxq 'PASS: indexed JSON Option lifecycle' \
        "$WORK_DIR/indexed_json-$profile.log"
done

for profile in fast release; do
    ir_dir="$WORK_DIR/same_module-$profile-ir"
    option_allocations=$(rg --no-ignore -c \
        'seen_pool_alloc\(i64 24\).*Option<' "$ir_dir" | \
        awk -F: '{ total += $NF } END { print total + 0 }')
    option_releases=$(rg --no-ignore -c \
        'seen_pool_free\(ptr .*i64 24\)' "$ir_dir" | \
        awk -F: '{ total += $NF } END { print total + 0 }')
    [ "$option_allocations" -gt 0 ] || {
        echo "FAIL: $profile Option allocation ABI was not emitted" >&2
        exit 1
    }
    [ "$option_releases" -gt 0 ] || {
        echo "FAIL: $profile Option release ABI was not emitted" >&2
        exit 1
    }
    if rg --no-ignore -q \
        'seen_pool_alloc\(i64 (2|16)\).*Option<' "$ir_dir"; then
        echo "FAIL: $profile emitted a non-canonical Option allocation" >&2
        exit 1
    fi
done

ASAN_LOG="$WORK_DIR/same-module-asan.log"
run_compiler compile "$FIXTURES/same_module.seen" \
    "$WORK_DIR/same-module-asan" --fast --sanitize=address --no-cache \
    --frozen --target-cpu=x86-64 --jobs 1 --opt-jobs 1 \
    >"$ASAN_LOG" 2>&1
timeout --foreground --kill-after=5s 120s \
    bash "$ASAN_RUNNER" --target-root "$WORK_DIR" \
        --compile-log "$ASAN_LOG" -- "$WORK_DIR/same-module-asan" \
        >>"$ASAN_LOG" 2>&1
grep -Fxq 'PASS: same-module Option heap-box layout' "$ASAN_LOG"

echo 'PASS: Option allocation/free layout and size-class integrity contract'
