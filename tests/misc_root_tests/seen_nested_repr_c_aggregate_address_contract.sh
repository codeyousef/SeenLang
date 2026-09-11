#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-nested-repr-c-aggregate-address
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
FIXTURE="$ROOT_DIR/tests/fixtures/fel-1575/nested_aggregate_address.seen"
WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/nested-aggregate-address.XXXXXX")"

cleanup() {
    local status=$?
    case "$WORK_DIR" in
        "$SEEN_ARTIFACT_ROOT"/nested-aggregate-address.*)
            if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    rm -rf -- "$WORK_DIR"
            else
                echo "Preserved nested aggregate address artifacts: $WORK_DIR" >&2
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

for profile in fast release; do
    output="$WORK_DIR/nested-address-$profile"
    ir_dir="$WORK_DIR/ir-$profile"
    log="$WORK_DIR/$profile.log"
    flags=(--no-cache --frozen --target-cpu=x86-64 --jobs 1 --opt-jobs 1
        --emit-module-ir-dir "$ir_dir")
    if [ "$profile" = release ]; then
        flags+=(--release --lto=thin)
    else
        flags+=(--fast)
    fi
    run_compiler check "$FIXTURE" >"$log" 2>&1
    run_compiler compile "$FIXTURE" "$output" "${flags[@]}" >>"$log" 2>&1
    timeout --foreground --kill-after=5s 60s "$output" >>"$log" 2>&1
done

IR="$WORK_DIR/nested-address.ll"
find "$WORK_DIR/ir-release" -maxdepth 1 -type f -name '*.ll' -print0 |
    sort -z | xargs -0 cat >"$IR"
grep -Eq 'getelementptr inbounds \{ i64 \}, ptr %[^,]+, i32 0, i32 0' "$IR"
grep -Eq 'getelementptr inbounds \{ %LocalFixedAggregate \}, ptr %[^,]+, i32 0, i32 0' "$IR"
grep -Eq 'ptrtoint ptr %[^ ]+ to i64 ; &member .* address of field storage' "$IR"
grep -Eq 'call i32 @memcmp\(i64 %[^,]+, i64 %[^,]+, i64 4\)' "$IR"
grep -Eq 'call i64 @readImportedHigh\(i64 %[^)]+\)' "$IR"
grep -Eq 'define i32 @ProjectionSelectionPlan_compareAlgorithm\(' "$IR"
if grep -Pzq 'getelementptr inbounds \{ %(Local|Imported)FixedAggregate \},[^\n]*\n  %[^ ]+ = load i64, ptr ' "$IR"; then
    echo "FAIL: nested repr(C) field storage was loaded as a class handle" >&2
    exit 1
fi

echo "PASS: FEL-1575 nested repr(C) aggregate address contract"
