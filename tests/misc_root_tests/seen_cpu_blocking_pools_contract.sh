#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-cpu-blocking-pools
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/cpu-blocking-pools.XXXXXX")"
source "$ROOT_DIR/scripts/report_fixture_failure_logs.sh"
cleanup() {
    local status=$?
    if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$TMP_DIR" in
            "$SEEN_ARTIFACT_ROOT"/cpu-blocking-pools.*) rm -rf -- "$TMP_DIR" ;;
            *) status=1 ;;
        esac
    else
        echo "Preserved executor artifacts: $TMP_DIR" >&2
        seen_report_fixture_failure_logs "$TMP_DIR"
    fi
    exit "$status"
}
trap cleanup EXIT

POOL_COMPILE_FLAGS=()
compile_case() {
    local profile=$1
    local name=$2
    local source=$3
    local flags=(--fast)
    if [ "$profile" = release ]; then flags=(--release --lto=thin); fi
    local log="$TMP_DIR/$name-$profile.log"
    bash "$ATTESTED_SEEN" "$COMPILER" compile \
        "$source" "$TMP_DIR/$name-$profile" \
        --no-cache --frozen --target-cpu=x86-64 \
        "${flags[@]}" "${POOL_COMPILE_FLAGS[@]}" >"$log" 2>&1
    timeout --foreground --kill-after=5s 120s "$TMP_DIR/$name-$profile" >>"$log" 2>&1
}

for profile in fast release; do
    compile_case "$profile" lifecycle "$ROOT_DIR/seen_std/tests/async/async-002c.seen"
    compile_case "$profile" adverse "$ROOT_DIR/seen_std/tests/async/async-002c-adverse.seen"
    for case_name in happy partial invalid limit cancel cleanup; do
        rg -Fq "PASS: ASYNC-002C_$case_name" "$TMP_DIR/lifecycle-$profile.log"
    done
done

POOL_COMPILE_FLAGS=(--coverage)
for name in lifecycle adverse; do
    source="$ROOT_DIR/seen_std/tests/async/async-002c.seen"
    if [ "$name" = adverse ]; then source="$ROOT_DIR/seen_std/tests/async/async-002c-adverse.seen"; fi
    export LLVM_PROFILE_FILE="$TMP_DIR/$name.profraw"
    compile_case fast "coverage-$name" "$source"
done
source "$ROOT_DIR/scripts/source_coverage_fixture_support.sh"
seen_verify_source_coverage "$TMP_DIR/pools" \
    --fixture "$TMP_DIR/coverage-lifecycle-fast" "$TMP_DIR/lifecycle.profraw" \
    --fixture "$TMP_DIR/coverage-adverse-fast" "$TMP_DIR/adverse.profraw" \
    --source seen_std/src/async/pools.seen \
    --source seen_std/src/async/pool_placement.seen

if bash "$ATTESTED_SEEN" "$COMPILER" check \
    "$ROOT_DIR/tests/fixtures/async/unsafe_pool.seen" >"$TMP_DIR/unsafe.log" 2>&1; then
    echo "FAIL: worker pool admitted a borrowed raw pointer" >&2
    exit 1
fi
rg -Fq E_TYPE_BOUND "$TMP_DIR/unsafe.log"
echo "PASS: persistent CPU/blocking pool ownership in fast and release/ThinLTO"
