#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
F="$ROOT/tests/fixtures/test-002d"
C="$ROOT/scripts/check_leak_soak_evidence.py"
B="$ROOT/scripts/benchmark_leak_soak_evidence.py"
V="$ROOT/scripts/measure_leak_soak_coverage.py"
fail(){ echo "FAIL: TEST-002D contract: $*" >&2; exit 1; }
mkdir -p "$ROOT/.seen"
python3 -m py_compile "$C" "$B" "$V" "$ROOT/tests/runner/test_leak_soak_evidence_unit.py" || fail syntax
python3 "$ROOT/tests/runner/test_leak_soak_evidence_unit.py" >/dev/null || fail unit
python3 "$V" || fail coverage
python3 "$C" --evidence "$F/happy/evidence.json" >"$ROOT/.seen/test-002d-canonical.json" || fail TEST-002D_happy
cmp -s "$F/happy/evidence.json" "$ROOT/.seen/test-002d-canonical.json" || fail canonical
if python3 "$C" --evidence "$F/invalid/evidence.json" >/dev/null 2>"$ROOT/.seen/test-002d-invalid.err"; then fail TEST-002D_invalid; fi
grep -Fq test.002d.invalid "$ROOT/.seen/test-002d-invalid.err" || fail invalid-code
if python3 "$C" --evidence "$F/limit/evidence.json" >/dev/null 2>"$ROOT/.seen/test-002d-limit.err"; then fail TEST-002D_limit; fi
grep -Fq test.002d.limit "$ROOT/.seen/test-002d-limit.err" || fail limit-code
status=0; python3 "$C" --evidence "$F/cancel/evidence.json" --test-cancel-after-read >/dev/null 2>"$ROOT/.seen/test-002d-cancel.err" || status=$?
[ "$status" -eq 130 ] || fail TEST-002D_cancel
python3 "$C" --evidence "$F/happy/evidence.json" --fuzz-seconds "${SEEN_TEST_002D_FUZZ_SECONDS:-1}" --seed 1101 >/dev/null 2>"$ROOT/.seen/test-002d-fuzz.err" || fail fuzz
grep -Fq seed=1101 "$ROOT/.seen/test-002d-fuzz.err" || fail fuzz-seed
python3 "$B" "$F/happy/evidence.json" "$F/happy/benchmark.json" | grep -Fq 'warmups=5 samples=30 status=pass' || fail benchmark
[ -f "$ROOT/compiler_seen/src/testing/leak.seen" ] || fail native-policy
[ -f "$ROOT/compiler_seen/tests/test_002d_leak_soak.seen" ] || fail native-test
[ -f "$ROOT/compiler_seen/examples/test_leak_soak.seen" ] || fail example
[ -f "$ROOT/tests/fixtures/soak/test_002d_leak_soak.seen" ] || fail soak
grep -Fq seen-test-leak-soak-v1 "$ROOT/schemas/compatibility-manifest.schema.json" || fail compatibility
[ -z "$(find "$ROOT/.seen" -maxdepth 1 -name '.leak-soak-*' -print -quit)" ] || fail TEST-002D_cleanup
[ -z "$(jobs -pr)" ] || fail TEST-002D_cleanup
echo "PASS: leak and soak resource-provider lifecycle"
