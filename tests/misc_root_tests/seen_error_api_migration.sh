#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CHECK="$ROOT_DIR/scripts/check_error_api_migration.py"
BENCHMARK="$ROOT_DIR/scripts/benchmark_error_api_migration.py"
FIXTURES="$ROOT_DIR/tests/fixtures/err-001c"
NATIVE_TEST="$ROOT_DIR/seen_std/tests/error/err_001c_error_api_migration.seen"

fail() { echo "FAIL: ERR-001C contract: $*" >&2; exit 1; }

python3 -m py_compile "$CHECK" "$BENCHMARK" \
    "$ROOT_DIR/tests/runner/test_error_api_migration_unit.py" || fail "Python syntax"
python3 "$ROOT_DIR/tests/runner/test_error_api_migration_unit.py" >/dev/null || fail "unit matrix"
python3 "$CHECK" --validate "$FIXTURES/happy/contract.json" --root "$ROOT_DIR" \
    >/dev/null || fail "ERR-001C_happy"
if python3 "$CHECK" --validate "$FIXTURES/invalid/contract.json" \
    >/dev/null 2>"$ROOT_DIR/.seen/err-001c-invalid.err"; then fail "ERR-001C_invalid"; fi
grep -Fq err.001c.invalid "$ROOT_DIR/.seen/err-001c-invalid.err" || fail "invalid diagnostic"
if python3 "$CHECK" --validate "$FIXTURES/limit/contract.json" --max-bytes 1 \
    >/dev/null 2>"$ROOT_DIR/.seen/err-001c-limit.err"; then fail "ERR-001C_limit"; fi
grep -Fq err.001c.limit "$ROOT_DIR/.seen/err-001c-limit.err" || fail "limit diagnostic"
status=0
python3 "$CHECK" --validate "$FIXTURES/cancel/contract.json" \
    --test-cancel-after-read >/dev/null 2>"$ROOT_DIR/.seen/err-001c-cancel.err" || status=$?
[ "$status" -eq 130 ] || fail "ERR-001C_cancel"
python3 "$CHECK" --validate "$FIXTURES/happy/contract.json" \
    --fuzz-seconds "${SEEN_ERR_001C_FUZZ_SECONDS:-1}" --seed 1101 \
    >/dev/null 2>"$ROOT_DIR/.seen/err-001c-fuzz.err" || fail "fuzz"
grep -Fq seed=1101 "$ROOT_DIR/.seen/err-001c-fuzz.err" || fail "fuzz seed"
python3 "$BENCHMARK" "$FIXTURES/happy/contract.json" \
    "$FIXTURES/happy/benchmark.json" | \
    grep -Fq 'warmups=5 samples=30 status=pass' || fail "5/30/5 benchmark"
for symbol in seenFailure typedSeenErrorInContext; do
    grep -Rq "$symbol" "$ROOT_DIR/seen_std/src" || fail "native API $symbol"
done
for name in ERR-001C_happy ERR-001C_invalid ERR-001C_limit ERR-001C_cancel ERR-001C_cleanup; do
    grep -Fq "$name" "$NATIVE_TEST" || fail "native case $name"
done
grep -Fq 'seen_std/tests/error/err_001c_error_api_migration.seen' \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" || fail "Stage1 wiring"
grep -Fq 'seen-error-api-migration-v1' \
    "$ROOT_DIR/schemas/compatibility-manifest.schema.json" || fail "compatibility binding"
grep -Fq 'String-error and sentinel migration' "$ROOT_DIR/docs/errors.md" || fail "documentation"
grep -Fq 'seen-error-api-migration-v1' "$ROOT_DIR/CHANGELOG.md" || fail "changelog"
[ -z "$(jobs -pr)" ] || fail "ERR-001C_cleanup"
echo "PASS: string-error Result APIs replaced by structured Seen errors"
