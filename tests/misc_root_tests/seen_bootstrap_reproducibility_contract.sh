#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
F="$ROOT/tests/fixtures/core-004a"; C="$ROOT/scripts/check_bootstrap_reproducibility.py"; G="$ROOT/scripts/certify_two_builder_bootstrap.py"; B="$ROOT/scripts/benchmark_bootstrap_reproducibility.py"; V="$ROOT/scripts/measure_bootstrap_reproducibility_coverage.py"
fail(){ echo "FAIL: CORE-004A contract: $*" >&2; exit 1; }
mkdir -p "$ROOT/.seen"
python3 -m py_compile "$C" "$G" "$B" "$V" "$ROOT/tests/runner/test_bootstrap_reproducibility_unit.py" || fail syntax
python3 "$ROOT/tests/runner/test_bootstrap_reproducibility_unit.py" >/dev/null || fail unit
python3 "$V" || fail coverage
python3 "$C" --evidence "$F/happy/evidence.json" >"$ROOT/.seen/core-004a-canonical.json" || fail CORE-004A_happy
cmp -s "$F/happy/evidence.json" "$ROOT/.seen/core-004a-canonical.json" || fail canonical
if python3 "$C" --evidence "$F/invalid/evidence.json" >/dev/null 2>"$ROOT/.seen/core-004a-invalid.err"; then fail CORE-004A_invalid; fi
grep -Fq core.004a.invalid "$ROOT/.seen/core-004a-invalid.err" || fail invalid-code
if python3 "$C" --evidence "$F/limit/evidence.json" >/dev/null 2>"$ROOT/.seen/core-004a-limit.err"; then fail CORE-004A_limit; fi
grep -Fq core.004a.limit "$ROOT/.seen/core-004a-limit.err" || fail limit-code
if python3 "$C" --evidence "$F/mismatch/evidence.json" >/dev/null 2>"$ROOT/.seen/core-004a-mismatch.err"; then fail CORE-004A_mismatch; fi
grep -Fq core.004a.mismatch "$ROOT/.seen/core-004a-mismatch.err" || fail mismatch-code
status=0; python3 "$C" --evidence "$F/cancel/evidence.json" --test-cancel-after-read >/dev/null 2>"$ROOT/.seen/core-004a-cancel.err" || status=$?
[ "$status" -eq 130 ] || fail CORE-004A_cancel
python3 "$C" --evidence "$F/happy/evidence.json" --fuzz-seconds "${SEEN_CORE_004A_FUZZ_SECONDS:-1}" --seed 1101 >/dev/null 2>"$ROOT/.seen/core-004a-fuzz.err" || fail fuzz
grep -Fq seed=1101 "$ROOT/.seen/core-004a-fuzz.err" || fail fuzz-seed
python3 "$B" "$F/happy/evidence.json" "$F/happy/benchmark.json" | grep -Fq 'warmups=5 samples=30 status=pass' || fail benchmark
python3 -m json.tool "$ROOT/releases/manifest.schema.json" >/dev/null || fail schema
[ -f "$ROOT/compiler_seen/src/release/reproducibility.seen" ] || fail native-policy
[ -f "$ROOT/compiler_seen/tests/reproducibility/core_004a_two_builder.seen" ] || fail native-test
[ -f "$ROOT/compiler_seen/examples/bootstrap_reproducibility.seen" ] || fail example
[ -f "$ROOT/tests/fixtures/soak/core_004a_reproducibility.seen" ] || fail soak
grep -Fq -- '--peer-stage3' "$ROOT/scripts/release_bootstrap_matrix.sh" || fail matrix-peer
grep -Fq -- '--reproducibility-output' "$ROOT/scripts/release_bootstrap_matrix.sh" || fail matrix-output
grep -Fq 'run_in_hard_memory_scope.sh' "$ROOT/scripts/release_bootstrap_matrix.sh" || fail containment
grep -Fq seen-bootstrap-reproducibility-v1 "$ROOT/schemas/compatibility-manifest.schema.json" || fail compatibility
[ -z "$(find "$ROOT/.seen" -maxdepth 1 -name '.two-builder-*' -print -quit)" ] || fail CORE-004A_cleanup
[ -z "$(jobs -pr)" ] || fail CORE-004A_cleanup
echo "PASS: reproducible two-builder bootstrap certification"
