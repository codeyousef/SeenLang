#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CHECK="$ROOT_DIR/scripts/check_error_policy.py"
BENCHMARK="$ROOT_DIR/scripts/benchmark_error_policy.py"
FIXTURES="$ROOT_DIR/tests/fixtures/err-001d"
NATIVE_TEST="$ROOT_DIR/seen_std/tests/error/err_001d_error_policy.seen"
fail(){ echo "FAIL: ERR-001D contract: $*" >&2; exit 1; }

python3 -m py_compile "$CHECK" "$BENCHMARK" "$ROOT_DIR/tests/runner/test_error_policy_unit.py" || fail "Python syntax"
python3 "$ROOT_DIR/tests/runner/test_error_policy_unit.py" >/dev/null || fail "unit matrix"
python3 "$CHECK" --validate "$FIXTURES/happy/contract.json" | grep -Fq '"disposition":"retry"' || fail "ERR-001D_happy"
if python3 "$CHECK" --validate "$FIXTURES/invalid/contract.json" >/dev/null 2>"$ROOT_DIR/.seen/err-001d-invalid.err"; then fail "ERR-001D_invalid"; fi
grep -Fq err.001d.invalid "$ROOT_DIR/.seen/err-001d-invalid.err" || fail "invalid diagnostic"
if python3 "$CHECK" --validate "$FIXTURES/limit/contract.json" >/dev/null 2>"$ROOT_DIR/.seen/err-001d-limit.err"; then fail "ERR-001D_limit"; fi
grep -Fq err.001d.limit "$ROOT_DIR/.seen/err-001d-limit.err" || fail "limit diagnostic"
status=0; python3 "$CHECK" --validate "$FIXTURES/cancel/contract.json" --test-cancel-after-read >/dev/null 2>"$ROOT_DIR/.seen/err-001d-cancel.err" || status=$?
[ "$status" -eq 130 ] || fail "ERR-001D_cancel"
python3 "$CHECK" --validate "$FIXTURES/happy/contract.json" --fuzz-seconds "${SEEN_ERR_001D_FUZZ_SECONDS:-1}" --seed 1101 >/dev/null 2>"$ROOT_DIR/.seen/err-001d-fuzz.err" || fail "fuzz"
grep -Fq seed=1101 "$ROOT_DIR/.seen/err-001d-fuzz.err" || fail "fuzz seed"
python3 "$BENCHMARK" "$FIXTURES/happy/contract.json" "$FIXTURES/happy/benchmark.json" | grep -Fq 'warmups=5 samples=30 status=pass' || fail "5/30/5 benchmark"
for symbol in ErrorDisposition ErrorPolicyDecision classifySeenError errorDispositionName; do grep -Fq "$symbol" "$ROOT_DIR/seen_std/src/core/error.seen" || fail "native symbol $symbol"; done
for name in ERR-001D_happy ERR-001D_invalid ERR-001D_limit ERR-001D_cancel ERR-001D_cleanup; do grep -Fq "$name" "$NATIVE_TEST" || fail "native case $name"; done
grep -Fq 'seen_std/tests/error/err_001d_error_policy.seen' "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" || fail "Stage1 wiring"
grep -Fq seen-error-policy-v1 "$ROOT_DIR/schemas/compatibility-manifest.schema.json" || fail "compatibility binding"
grep -Fq 'Retry cancellation exhaustion and redaction' "$ROOT_DIR/docs/errors.md" || fail "documentation"
grep -Fq seen-error-policy-v1 "$ROOT_DIR/CHANGELOG.md" || fail "changelog"
[ -z "$(jobs -pr)" ] || fail "ERR-001D_cleanup"
echo "PASS: retry cancellation exhaustion and redaction policy"
