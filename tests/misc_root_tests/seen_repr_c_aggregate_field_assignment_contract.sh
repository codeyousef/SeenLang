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
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/repr-c-field.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=repr-c-field
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
aggregate="$ROOT_DIR/tests/fixtures/fel-1574/aggregate_field_assignment.seen"
local_store="$ROOT_DIR/tests/codegen/test_repr_c_local_field_store.seen"
run_compiler check "$aggregate" > "$TMP_DIR/check-aggregate.log" 2>&1
run_compiler check "$local_store" > "$TMP_DIR/check-local.log" 2>&1
for profile in fast release undefined address; do
    compile_case "$profile" aggregate "$aggregate"
    compile_case "$profile" local_store "$local_store"
    rg -Fq 'PASS: native repr(C) local and nested field stores' "$TMP_DIR/local_store-$profile.log"
done

IR="$TMP_DIR/aggregate-field.ll"
find "$TMP_DIR/aggregate-release-ir" -maxdepth 1 -type f -name '*.ll' -print0 |
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
