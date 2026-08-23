#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
F="$ROOT/tests/fixtures/p0-gate0-001"; C="$ROOT/scripts/check_gate0_certification.py"
B="$ROOT/scripts/benchmark_gate0_certification.py"; V="$ROOT/scripts/measure_gate0_certification_coverage.py"
WORK="$ROOT/.seen/p0-gate0-001-contract"
fail(){ echo "FAIL: P0-GATE0-001 contract: $*" >&2; exit 1; }
rm -rf -- "$WORK"; mkdir -p -- "$WORK"; trap 'rm -rf -- "$WORK"' EXIT
python3 -m py_compile "$C" "$B" "$V" "$ROOT/tests/runner/test_gate0_certification_unit.py" || fail syntax
python3 "$ROOT/tests/runner/test_gate0_certification_unit.py" >/dev/null || fail unit
python3 "$V" || fail coverage
python3 "$C" --evidence "$F/happy/evidence.json" >"$WORK/canonical.json" || fail P0-GATE0-001_happy
cmp -s "$F/happy/evidence.json" "$WORK/canonical.json" || fail canonical
if python3 "$C" --evidence "$F/invalid/evidence.json" >/dev/null 2>"$WORK/invalid.err"; then fail P0-GATE0-001_invalid; fi
grep -Fq p0.gate0.001.invalid "$WORK/invalid.err" || fail invalid-code
if python3 "$C" --evidence "$F/limit/evidence.json" >/dev/null 2>"$WORK/limit.err"; then fail P0-GATE0-001_limit; fi
grep -Fq p0.gate0.001.limit "$WORK/limit.err" || fail limit-code
status=0; python3 "$C" --evidence "$F/cancel/evidence.json" --test-cancel-after-read >/dev/null 2>"$WORK/cancel.err" || status=$?
[[ "$status" -eq 130 ]] || fail P0-GATE0-001_cancel
python3 "$C" --evidence "$F/happy/evidence.json" --fuzz-seconds "${SEEN_P0_GATE0_CONTRACT_FUZZ_SECONDS:-1}" --seed 1101 >/dev/null 2>"$WORK/fuzz.err" || fail fuzz
grep -Fq seed=1101 "$WORK/fuzz.err" || fail fuzz-seed
python3 "$B" "$F/happy/evidence.json" "$F/happy/benchmark.json" | grep -Fq 'warmups=5 samples=30 status=pass' || fail benchmark
[[ -f "$ROOT/compiler_seen/src/release/gate0_certification.seen" ]] || fail native-policy
[[ -f "$ROOT/compiler_seen/tests/release/p0_gate0_001_certification.seen" ]] || fail native-test
[[ -f "$ROOT/compiler_seen/examples/gate0_certification.seen" ]] || fail example
[[ -f "$ROOT/schemas/gate0-certification.schema.json" ]] || fail schema
[[ -x "$ROOT/scripts/certify_gate0_clean_checkout.sh" ]] || fail clean-driver
grep -Fq 'safe_rebuild.sh" --tier full' "$ROOT/scripts/certify_gate0_clean_checkout.sh" || fail pinned-build
grep -Fq 'run_with_project_artifacts.sh' "$ROOT/scripts/certify_gate0_clean_checkout.sh" || fail artifact-wrapper
grep -Fq 'Gate 0: running the canonical Seen test against the installed compiler' \
    "$ROOT/scripts/certify_gate0_clean_checkout.sh" || fail post-build-canonical-test
grep -Fq 'prlimit --as=' "$ROOT/scripts/certify_gate0_clean_checkout.sh" || fail test-vmem
grep -Fq "grep -Eq 'Shared library: \\[(libSDL3\\.so|libvulkan\\.so)'" \
    "$ROOT/scripts/certify_gate0_clean_checkout.sh" || fail optional-runtime-dependency
grep -Fq 'getelementptr inbounds nuw i64' \
    "$ROOT/scripts/seen_prebuild_gates.sh" || fail llvm-dialect-preflight
grep -Fq seen-gate0-certification-v1 "$ROOT/schemas/compatibility-manifest.schema.json" || fail compatibility
"$ROOT/tests/misc_root_tests/seen_native_boundaries_ledger.sh" >/dev/null || fail native-ledger
[[ -z "$(jobs -pr)" ]] || fail P0-GATE0-001_cleanup
echo "PASS: P0-GATE0-001 certification contract"
