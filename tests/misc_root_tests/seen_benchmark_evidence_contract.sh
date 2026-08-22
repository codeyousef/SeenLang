#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
F="$ROOT/tests/fixtures/test-002c"
C="$ROOT/scripts/check_benchmark_evidence.py"
B="$ROOT/scripts/benchmark_benchmark_evidence.py"
V="$ROOT/scripts/measure_benchmark_evidence_coverage.py"
fail(){ echo "FAIL: TEST-002C contract: $*" >&2; exit 1; }
mkdir -p "$ROOT/.seen"
python3 -m py_compile "$C" "$B" "$V" "$ROOT/tests/runner/test_benchmark_evidence_unit.py" || fail syntax
python3 "$ROOT/tests/runner/test_benchmark_evidence_unit.py" >/dev/null || fail unit
python3 "$V" || fail coverage
python3 "$C" --evidence "$F/happy/evidence.json" >"$ROOT/.seen/test-002c-canonical.json" || fail TEST-002C_happy
cmp -s "$F/happy/evidence.json" "$ROOT/.seen/test-002c-canonical.json" || fail canonical
if python3 "$C" --evidence "$F/invalid/evidence.json" >/dev/null 2>"$ROOT/.seen/test-002c-invalid.err"; then fail TEST-002C_invalid; fi
grep -Fq test.002c.invalid "$ROOT/.seen/test-002c-invalid.err" || fail invalid-code
if python3 "$C" --evidence "$F/limit/evidence.json" >/dev/null 2>"$ROOT/.seen/test-002c-limit.err"; then fail TEST-002C_limit; fi
grep -Fq test.002c.limit "$ROOT/.seen/test-002c-limit.err" || fail limit-code
status=0; python3 "$C" --evidence "$F/cancel/evidence.json" --test-cancel-after-read >/dev/null 2>"$ROOT/.seen/test-002c-cancel.err" || status=$?
[ "$status" -eq 130 ] || fail TEST-002C_cancel
python3 "$C" --evidence "$F/happy/evidence.json" --fuzz-seconds "${SEEN_TEST_002C_FUZZ_SECONDS:-1}" --seed 1101 >/dev/null 2>"$ROOT/.seen/test-002c-fuzz.err" || fail fuzz
grep -Fq seed=1101 "$ROOT/.seen/test-002c-fuzz.err" || fail fuzz-seed
python3 "$B" "$F/happy/evidence.json" "$F/happy/benchmark.json" | grep -Fq 'warmups=5 samples=30 status=pass' || fail benchmark
[ -f "$ROOT/compiler_seen/src/testing/benchmark.seen" ] || fail native-policy
[ -f "$ROOT/compiler_seen/tests/test_002c_benchmark_evidence.seen" ] || fail native-test
[ -f "$ROOT/compiler_seen/examples/test_benchmark_evidence.seen" ] || fail example
[ -f "$ROOT/tests/fixtures/soak/test_002c_benchmark_evidence.seen" ] || fail soak
grep -Fq seen-benchmark-evidence-v1 "$ROOT/schemas/compatibility-manifest.schema.json" || fail compatibility
[ -z "$(jobs -pr)" ] || fail TEST-002C_cleanup
echo "PASS: benchmark baseline and regression evidence lifecycle"
