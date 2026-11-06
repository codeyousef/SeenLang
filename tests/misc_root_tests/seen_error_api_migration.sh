#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CHECK="$ROOT_DIR/scripts/check_error_api_migration.py"
BENCHMARK="$ROOT_DIR/scripts/benchmark_error_api_migration.py"
FIXTURES="$ROOT_DIR/tests/fixtures/err-001c"

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
    "$FIXTURES/happy/benchmark.json" || fail "5/30/5 benchmark"
[ -z "$(jobs -pr)" ] || fail "ERR-001C_cleanup"
echo "PASS: string-error Result APIs replaced by structured Seen errors"
