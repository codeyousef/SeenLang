#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"; F="$ROOT/tests/fixtures/test-002a"; C="$ROOT/scripts/check_test_instrumentation.py"; B="$ROOT/scripts/benchmark_test_instrumentation.py"; fail(){ echo "FAIL: TEST-002A contract: $*" >&2; exit 1; }
python3 -m py_compile "$C" "$B" "$ROOT/tests/runner/test_test_instrumentation_unit.py" || fail syntax
python3 "$ROOT/tests/runner/test_test_instrumentation_unit.py" >/dev/null || fail unit
python3 "$C" --evidence "$F/happy/evidence.json" >/dev/null || fail TEST-002A_happy
if python3 "$C" --evidence "$F/invalid/evidence.json" >/dev/null 2>"$ROOT/.seen/test-002a-invalid.err"; then fail TEST-002A_invalid; fi
grep -Fq test.002a.invalid "$ROOT/.seen/test-002a-invalid.err" || fail invalid-code
if python3 "$C" --evidence "$F/limit/evidence.json" >/dev/null 2>"$ROOT/.seen/test-002a-limit.err"; then fail TEST-002A_limit; fi
grep -Fq test.002a.limit "$ROOT/.seen/test-002a-limit.err" || fail limit-code
status=0; python3 "$C" --evidence "$F/cancel/evidence.json" --test-cancel-after-read >/dev/null 2>"$ROOT/.seen/test-002a-cancel.err" || status=$?; [ "$status" -eq 130 ] || fail TEST-002A_cancel
python3 "$C" --evidence "$F/happy/evidence.json" --fuzz-seconds "${SEEN_TEST_002A_FUZZ_SECONDS:-1}" --seed 1101 >/dev/null 2>"$ROOT/.seen/test-002a-fuzz.err" || fail fuzz
grep -Fq seed=1101 "$ROOT/.seen/test-002a-fuzz.err" || fail fuzz-seed
python3 "$B" "$F/happy/evidence.json" "$F/happy/benchmark.json" | grep -Fq 'warmups=5 samples=30 status=pass' || fail benchmark
for profile in coverage sanitizer-undefined sanitizer-thread; do grep -Fq "$profile" "$ROOT/compiler_seen/src/testing/runner.seen" || fail "profile $profile"; done
for unsupported in sanitizer-address sanitizer-memory; do
    if grep -Fq "$unsupported" "$ROOT/compiler_seen/src/testing/runner.seen"; then fail "unsupported profile $unsupported"; fi
done
grep -Fq 'instrumentationFlags' "$ROOT/compiler_seen/src/testing/cli_runner.seen" || fail execution-wiring
grep -Fq -- '--no-fork' "$ROOT/compiler_seen/src/testing/cli_runner.seen" || fail serialized-compile
grep -Fq -- '--threads=1,--thinlto-jobs=1' "$ROOT/compiler_seen/src/main_compiler.seen" || fail serialized-link
grep -Fq 'llvm-profdata merge' "$ROOT/compiler_seen/src/testing/cli_runner.seen" || fail coverage-merge
grep -Fq 'llvm-cov report' "$ROOT/compiler_seen/src/testing/cli_runner.seen" || fail coverage-report
grep -Fq 'workingRoot + "/.seen_cache/test/run"' "$ROOT/compiler_seen/src/testing/cli_runner.seen" || fail working-root-artifacts
grep -Fq 'test_002a_gpu_emitters.seen' "$ROOT/compiler_seen/src/testing/cli_runner.seen" || fail gpu-emitter-wiring
grep -Fq 'hardwareExecuted' "$ROOT/compiler_seen/src/testing/instrumentation.seen" || fail hardware-separation
for module in fuzz benchmark; do [ -f "$ROOT/compiler_seen/src/testing/$module.seen" ] || fail "native-$module"; done
[ -f "$ROOT/tests/fixtures/soak/test_002a_instrumentation.seen" ] || fail soak-fixture
grep -Fq 'derive-execution' "$ROOT/scripts/run_test_instrumentation_acceptance.sh" || fail execution-acceptance
grep -Fq 'compiler_seen/tests/test_002a_instrumentation.seen' "$ROOT/scripts/seen_stage1_acceptance.sh" || fail stage1
grep -Fq seen-test-instrumentation-v1 "$ROOT/schemas/compatibility-manifest.schema.json" || fail compatibility
[ -z "$(jobs -pr)" ] || fail TEST-002A_cleanup
echo "PASS: sanitizer and coverage test modes"
