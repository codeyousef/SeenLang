#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"; F="$ROOT/tests/fixtures/test-002a"; C="$ROOT/scripts/check_test_instrumentation.py"; B="$ROOT/scripts/benchmark_test_instrumentation.py"; fail(){ echo "FAIL: TEST-002A contract: $*" >&2; exit 1; }
python3 -m py_compile "$C" "$B" "$ROOT/tests/runner/test_test_instrumentation_unit.py" || fail syntax
python3 "$ROOT/tests/runner/test_test_instrumentation_unit.py" >/dev/null || fail unit
python3 "$ROOT/tests/runner/test_source_coverage_thresholds_unit.py" >/dev/null || fail source-coverage-threshold-unit
python3 "$C" --evidence "$F/happy/evidence.json" >/dev/null || fail TEST-002A_happy
if python3 "$C" --evidence "$F/invalid/evidence.json" >/dev/null 2>"$ROOT/.seen/test-002a-invalid.err"; then fail TEST-002A_invalid; fi
grep -Fq test.002a.invalid "$ROOT/.seen/test-002a-invalid.err" || fail invalid-code
if python3 "$C" --evidence "$F/limit/evidence.json" >/dev/null 2>"$ROOT/.seen/test-002a-limit.err"; then fail TEST-002A_limit; fi
grep -Fq test.002a.limit "$ROOT/.seen/test-002a-limit.err" || fail limit-code
status=0; python3 "$C" --evidence "$F/cancel/evidence.json" --test-cancel-after-read >/dev/null 2>"$ROOT/.seen/test-002a-cancel.err" || status=$?; [ "$status" -eq 130 ] || fail TEST-002A_cancel
python3 "$C" --evidence "$F/happy/evidence.json" --fuzz-seconds "${SEEN_TEST_002A_FUZZ_SECONDS:-1}" --seed 1101 >/dev/null 2>"$ROOT/.seen/test-002a-fuzz.err" || fail fuzz
python3 "$B" "$F/happy/evidence.json" "$F/happy/benchmark.json" >/dev/null || fail benchmark
echo "PASS: instrumentation evidence validation"
