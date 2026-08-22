#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"; F="$ROOT/tests/fixtures/test-002b"; C="$ROOT/scripts/check_fuzz_corpus.py"; B="$ROOT/scripts/benchmark_fuzz_corpus.py"; V="$ROOT/scripts/measure_fuzz_corpus_coverage.py"; fail(){ echo "FAIL: TEST-002B contract: $*" >&2; exit 1; }
python3 -m py_compile "$C" "$B" "$V" "$ROOT/tests/runner/test_fuzz_corpus_unit.py" || fail syntax
python3 "$ROOT/tests/runner/test_fuzz_corpus_unit.py" >/dev/null || fail unit
python3 "$V" || fail coverage
python3 "$C" --corpus "$F/happy/corpus.json" --exercise-minimizer --output "$ROOT/.seen/test-002b-canonical.json" >/dev/null || fail TEST-002B_happy
cmp -s "$F/happy/corpus.json" "$ROOT/.seen/test-002b-canonical.json" || fail canonical
if python3 "$C" --corpus "$F/invalid/corpus.json" >/dev/null 2>"$ROOT/.seen/test-002b-invalid.err"; then fail TEST-002B_invalid; fi
grep -Fq test.002b.invalid "$ROOT/.seen/test-002b-invalid.err" || fail invalid-code
if python3 "$C" --corpus "$F/limit/corpus.json" >/dev/null 2>"$ROOT/.seen/test-002b-limit.err"; then fail TEST-002B_limit; fi
grep -Fq test.002b.limit "$ROOT/.seen/test-002b-limit.err" || fail limit-code
status=0; python3 "$C" --corpus "$F/cancel/corpus.json" --test-cancel-after-read >/dev/null 2>"$ROOT/.seen/test-002b-cancel.err" || status=$?; [ "$status" -eq 130 ] || fail TEST-002B_cancel
python3 "$C" --corpus "$F/happy/corpus.json" --fuzz-seconds "${SEEN_TEST_002B_FUZZ_SECONDS:-1}" --seed 1101 >/dev/null 2>"$ROOT/.seen/test-002b-fuzz.err" || fail fuzz
grep -Fq seed=1101 "$ROOT/.seen/test-002b-fuzz.err" || fail fuzz-seed
python3 "$B" "$F/happy/corpus.json" "$F/happy/benchmark.json" | grep -Fq 'warmups=5 samples=30 status=pass' || fail benchmark
[ -f "$ROOT/compiler_seen/src/testing/fuzz.seen" ] || fail native-policy
[ -f "$ROOT/compiler_seen/tests/test_002b_fuzz_corpus.seen" ] || fail native-test
[ -f "$ROOT/compiler_seen/examples/test_fuzz_corpus.seen" ] || fail example
grep -Fq seen-test-fuzz-corpus-v1 "$ROOT/schemas/compatibility-manifest.schema.json" || fail compatibility
[ -z "$(find "$ROOT/.seen" -maxdepth 1 -name '.fuzz-corpus-*' -print -quit)" ] || fail TEST-002B_cleanup
[ -z "$(jobs -pr)" ] || fail TEST-002B_cleanup
echo "PASS: fuzz corpus minimization and replay lifecycle"
