#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CHECK="$ROOT_DIR/scripts/check_error_contract.py"; FIXTURES="$ROOT_DIR/tests/fixtures/err-001a"
BENCHMARK="$ROOT_DIR/scripts/benchmark_error_contract.py"
ERROR="$ROOT_DIR/seen_std/src/core/error.seen"; CONTEXT="$ROOT_DIR/seen_std/src/core/operation_context.seen"
NATIVE_TEST="$ROOT_DIR/seen_std/tests/error/err_001a_error_contract.seen"
fail(){ echo "FAIL: ERR-001A contract: $*" >&2; exit 1; }
python3 -m py_compile "$CHECK" "$ROOT_DIR/tests/runner/test_error_contract_unit.py" || fail "Python syntax"
python3 "$ROOT_DIR/tests/runner/test_error_contract_unit.py" >/dev/null || fail "unit matrix"
python3 "$CHECK" --validate "$FIXTURES/happy/contract.json" >/dev/null || fail "ERR-001A_happy"
if python3 "$CHECK" --validate "$FIXTURES/invalid/contract.json" >/dev/null 2>"$ROOT_DIR/.seen/err-001a-invalid.err"; then fail "ERR-001A_invalid"; fi
grep -Fq err.001a.invalid "$ROOT_DIR/.seen/err-001a-invalid.err" || fail "invalid diagnostic"
if python3 "$CHECK" --validate "$FIXTURES/limit/contract.json" --max-bytes 1 >/dev/null 2>"$ROOT_DIR/.seen/err-001a-limit.err"; then fail "ERR-001A_limit"; fi
grep -Fq err.001a.limit "$ROOT_DIR/.seen/err-001a-limit.err" || fail "limit diagnostic"
status=0; python3 "$CHECK" --validate "$FIXTURES/cancel/contract.json" --test-cancel-after-read >/dev/null 2>"$ROOT_DIR/.seen/err-001a-cancel.err" || status=$?
[ "$status" -eq 130 ] || fail "ERR-001A_cancel"
python3 "$CHECK" --validate "$FIXTURES/happy/contract.json" --fuzz-seconds "${SEEN_ERR_001A_FUZZ_SECONDS:-1}" --seed 1101 >/dev/null 2>"$ROOT_DIR/.seen/err-001a-fuzz.err" || fail "fuzz"
grep -Fq seed=1101 "$ROOT_DIR/.seen/err-001a-fuzz.err" || fail "fuzz seed"
python3 "$BENCHMARK" "$FIXTURES/happy/contract.json" \
    "$FIXTURES/happy/benchmark.json" | \
    grep -Fq 'ceiling_ratio_ppm=3150000 warmups=5 samples=30 status=pass' || \
    fail "5/30/5 benchmark"
for symbol in AsciiString RetryClass RedactionClass SeenError validateSeenError renderSeenError; do grep -Fq "$symbol" "$ERROR" || fail "error API $symbol"; done
for symbol in OperationLimits OperationContext validateOperationContext validateSeenErrorInContext; do grep -Fq "$symbol" "$CONTEXT" || fail "context API $symbol"; done
for code in err.001a.invalid err.001a.limit err.001a.cancelled; do grep -Fq "$code" "$ERROR" "$CONTEXT" || fail "native diagnostic $code"; done
for name in ERR-001A_happy ERR-001A_invalid ERR-001A_limit ERR-001A_cancel ERR-001A_cleanup; do grep -Fq "$name" "$NATIVE_TEST" || fail "native case $name"; done
grep -Fq 'pub import core.error' "$ROOT_DIR/seen_std/src/error.seen" || fail "public error surface"
grep -Fq 'pub import core.operation_context' "$ROOT_DIR/seen_std/src/operation_context.seen" || fail "public context surface"
if rg -q 'class (AsciiString|SeenError|OperationLimits|OperationContext)|enum (RetryClass|RedactionClass)' "$ROOT_DIR/compiler_seen/src/release/compatibility.seen"; then fail "compiler-owned duplicate contract"; fi
grep -Fq 'seen_std/tests/error/err_001a_error_contract.seen' "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" || fail "Stage-1 wiring"
grep -Fq 'seen-error-v1' "$ROOT_DIR/schemas/compatibility-manifest.schema.json" || fail "compatibility binding"
grep -Fq 'Structured errors' "$ROOT_DIR/docs/errors.md" || fail "documentation"
grep -Fq 'seen-error-v1' "$ROOT_DIR/CHANGELOG.md" || fail "changelog"
[ -z "$(jobs -pr)" ] || fail "ERR-001A_cleanup"
echo "PASS: stable structured error and operation-context contract"
