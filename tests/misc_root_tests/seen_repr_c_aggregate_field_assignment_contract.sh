#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-repr-c-aggregate-field-assignment
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
FIXTURE="$ROOT_DIR/tests/fixtures/fel-1574/aggregate_field_assignment.seen"
WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/repr-c-field.XXXXXX")"

cleanup() {
    local status=$?
    case "$WORK_DIR" in
        "$SEEN_ARTIFACT_ROOT"/repr-c-field.*)
            if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    rm -rf -- "$WORK_DIR"
            else
                echo "Preserved repr(C) field artifacts: $WORK_DIR" >&2
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
    output="$WORK_DIR/aggregate-field-$profile"
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

IR="$WORK_DIR/aggregate-field.ll"
find "$WORK_DIR/ir-release" -maxdepth 1 -type f -name '*.ll' -print0 |
    sort -z | xargs -0 cat >"$IR"
grep -Eq '%AggregateOwner = type \{ %ImportedFieldRecord, %LocalFieldRecord \}' "$IR"
grep -Eq 'load %ImportedFieldRecord, ptr .* ; inline aggregate literal field assignment' "$IR"
grep -Eq 'load %LocalFieldRecord, ptr .* ; inline aggregate literal field assignment' "$IR"
grep -Eq 'store %ImportedFieldRecord %[^,]+, ptr ' "$IR"
grep -Eq 'store %LocalFieldRecord %[^,]+, ptr ' "$IR"
if grep -Eq '%AggregateOwner = type \{ i64' "$IR"; then
    echo "FAIL: aggregate class field collapsed to i64" >&2
    exit 1
fi
if grep -Eq 'store %(Imported|Local)FieldRecord %[^,]*ptr[^,]*, ptr ' "$IR"; then
    echo "FAIL: aggregate literal pointer stored as an inline value" >&2
    exit 1
fi

echo "PASS: FEL-1574 repr(C) aggregate class-field assignment contract"
