#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-result-aggregate-array-data
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
FIXTURE_ROOT="$ROOT_DIR/tests/fixtures/fel-1554-1555"
WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/result-aggregate-data.XXXXXX")"

cleanup() {
    local status=$?
    case "$WORK_DIR" in
        "$SEEN_ARTIFACT_ROOT"/result-aggregate-data.*)
            if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    rm -rf -- "$WORK_DIR"
            else
                echo "Preserved Result/Array artifacts: $WORK_DIR" >&2
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

compile_case() {
    local source_name=$1
    local profile=$2
    local output="$WORK_DIR/${source_name%.seen}-$profile"
    local ir_dir="$WORK_DIR/ir-${source_name%.seen}-$profile"
    local log="$WORK_DIR/${source_name%.seen}-$profile.log"
    local flags=(--no-cache --frozen --target-cpu=x86-64
        --jobs 1 --opt-jobs 1 --emit-module-ir-dir "$ir_dir")
    if [ "$profile" = release ]; then
        flags+=(--release --lto=thin)
    else
        flags+=(--fast)
    fi
    run_compiler check "$FIXTURE_ROOT/$source_name" >"$log" 2>&1
    run_compiler compile "$FIXTURE_ROOT/$source_name" "$output" \
        "${flags[@]}" >>"$log" 2>&1
    timeout --foreground --kill-after=5s 60s "$output" >>"$log" 2>&1
}

for profile in fast release; do
    compile_case result_aggregate.seen "$profile"
    compile_case array_data.seen "$profile"
done

RESULT_IR="$WORK_DIR/result.ll"
find "$WORK_DIR/ir-result_aggregate-release" -maxdepth 1 -type f \
    -name '*.ll' -print0 | sort -z | xargs -0 cat >"$RESULT_IR"
grep -Eq 'load %LocalView, ptr ' "$RESULT_IR"
grep -Eq 'load %AggregateView, ptr ' "$RESULT_IR"
if grep -Eq '= call i64 @Result_(unwrap|expect)\(' "$RESULT_IR"; then
    echo "FAIL: aggregate Result payload used the erased i64 ABI" >&2
    exit 1
fi

ARRAY_IR="$WORK_DIR/array.ll"
find "$WORK_DIR/ir-array_data-release" -maxdepth 1 -type f \
    -name '*.ll' -print0 | sort -z | xargs -0 cat >"$ARRAY_IR"
grep -Eq '= call i64 @seen_arr_data_ptr\(ptr ' "$ARRAY_IR"
if grep -Eq '@Array_data\(' "$ARRAY_IR"; then
    echo "FAIL: Array.data emitted the undefined Array_data symbol" >&2
    exit 1
fi

echo "PASS: FEL-1554/FEL-1555 aggregate Result and Array.data contract"
