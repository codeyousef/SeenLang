#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CHECK="$ROOT_DIR/scripts/check_typed_errors.py"
BENCHMARK="$ROOT_DIR/scripts/benchmark_typed_errors.py"
FIXTURES="$ROOT_DIR/tests/fixtures/err-001b"
ERROR="$ROOT_DIR/seen_std/src/core/error.seen"
CONTEXT="$ROOT_DIR/seen_std/src/core/operation_context.seen"
NATIVE_TEST="$ROOT_DIR/seen_std/tests/error/err_001b_typed_errors.seen"

fail() {
    echo "FAIL: ERR-001B contract: $*" >&2
    exit 1
}

python3 -m py_compile "$CHECK" "$BENCHMARK" \
    "$ROOT_DIR/tests/runner/test_typed_errors_unit.py" || fail "Python syntax"
python3 "$ROOT_DIR/tests/runner/test_typed_errors_unit.py" >/dev/null || fail "unit matrix"
python3 "$CHECK" --validate "$FIXTURES/happy/contract.json" | \
    grep -Fq '"code":"err.001b.network"' || fail "ERR-001B_happy"
if python3 "$CHECK" --validate "$FIXTURES/invalid/contract.json" \
    >/dev/null 2>"$ROOT_DIR/.seen/err-001b-invalid.err"; then
    fail "ERR-001B_invalid"
fi
grep -Fq err.001b.invalid "$ROOT_DIR/.seen/err-001b-invalid.err" || \
    fail "invalid diagnostic"
if python3 "$CHECK" --validate "$FIXTURES/limit/contract.json" --max-bytes 1 \
    >/dev/null 2>"$ROOT_DIR/.seen/err-001b-limit.err"; then
    fail "ERR-001B_limit"
fi
grep -Fq err.001b.limit "$ROOT_DIR/.seen/err-001b-limit.err" || \
    fail "limit diagnostic"
status=0
python3 "$CHECK" --validate "$FIXTURES/cancel/contract.json" \
    --test-cancel-after-read >/dev/null 2>"$ROOT_DIR/.seen/err-001b-cancel.err" || status=$?
[ "$status" -eq 130 ] || fail "ERR-001B_cancel"
python3 "$CHECK" --validate "$FIXTURES/happy/contract.json" \
    --fuzz-seconds "${SEEN_ERR_001B_FUZZ_SECONDS:-1}" --seed 1101 \
    >/dev/null 2>"$ROOT_DIR/.seen/err-001b-fuzz.err" || fail "fuzz"
grep -Fq seed=1101 "$ROOT_DIR/.seen/err-001b-fuzz.err" || fail "fuzz seed"
python3 "$BENCHMARK" "$FIXTURES/happy/contract.json" \
    "$FIXTURES/happy/benchmark.json" | \
    grep -Fq 'ceiling_ratio_ppm=3150000 warmups=5 samples=30 status=pass' || \
    fail "5/30/5 benchmark"
for symbol in SeenErrorKind TypedErrorSpec typedSeenError seenErrorKindName; do
    grep -Fq "$symbol" "$ERROR" || fail "typed API $symbol"
done
grep -Fq typedSeenErrorInContext "$CONTEXT" || fail "context API"
for kind in Os Io Process Network Timeout Cancelled Parse Resource; do
    grep -Fq "SeenErrorKind.$kind" "$NATIVE_TEST" || fail "native kind $kind"
done
for name in ERR-001B_happy ERR-001B_invalid ERR-001B_limit ERR-001B_cancel ERR-001B_cleanup; do
    grep -Fq "$name" "$NATIVE_TEST" || fail "native case $name"
done
grep -Fq 'seen_std/tests/error/err_001b_typed_errors.seen' \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" || fail "Stage1 wiring"
grep -Fq 'seen-typed-error-v1' "$ROOT_DIR/schemas/compatibility-manifest.schema.json" || \
    fail "compatibility binding"
grep -Fq 'Typed error categories' "$ROOT_DIR/docs/errors.md" || fail "documentation"
grep -Fq 'seen-typed-error-v1' "$ROOT_DIR/CHANGELOG.md" || fail "changelog"
[ -z "$(jobs -pr)" ] || fail "ERR-001B_cleanup"
echo "PASS: typed OS/I/O/process/network/timeout/cancel/parse/resource errors"
