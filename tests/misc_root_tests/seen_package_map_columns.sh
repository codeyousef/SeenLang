#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-package-map-columns
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/package-map-columns.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=package-map-columns
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
source "$ROOT_DIR/scripts/source_coverage_fixture_support.sh"
FIXTURES="$ROOT_DIR/tests/fixtures/test-002a/package_map_columns"
for profile in fast release undefined address; do
    compile_case "$profile" columns "$FIXTURES/main.seen"
done
FIXTURE_COMPILE_FLAGS=(--coverage)
export LLVM_PROFILE_FILE="$TMP_DIR/columns.profraw"
compile_case fast fuzz "$FIXTURES/fuzz.seen"
unset LLVM_PROFILE_FILE
seen_verify_source_coverage "$TMP_DIR/columns" \
    --fixture "$TMP_DIR/fuzz-fast" "$TMP_DIR/columns.profraw" \
    --source compiler_seen/src/imports/package_map.seen
python3 "$ROOT_DIR/scripts/check_source_coverage_thresholds.py" \
    "$TMP_DIR/columns-coverage.json" \
    --source compiler_seen/src/imports/package_map.seen \
    --min-lines 90 --min-branches 90 >"$TMP_DIR/columns-parser-coverage-summary.json"
echo 'PASS: package-map v2 native framing, four profiles and 60-second seed-1101 fuzz'
