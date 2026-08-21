#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
FIXTURES="$ROOT_DIR/tests/fixtures/core-rel-003"
CHECKER="$ROOT_DIR/scripts/check_release_optimization.py"
BENCHMARK="$ROOT_DIR/scripts/benchmark_release_optimization.py"
SOURCE="$ROOT_DIR/compiler_seen/src/release/release_optimization.seen"
COMPILER="$ROOT_DIR/compiler_seen/src/main_compiler.seen"
fail(){ echo "FAIL: release optimization contract: $*" >&2; exit 1; }
python3 -m py_compile "$CHECKER" "$BENCHMARK" "$ROOT_DIR/tests/misc_root_tests/seen_release_optimization_unit.py" || fail "Python syntax"
python3 "$ROOT_DIR/tests/misc_root_tests/seen_release_optimization_unit.py" >/dev/null || fail "unit matrix"
happy=$(python3 "$CHECKER" "$FIXTURES/happy/plan.json") || fail "CORE-REL-003_happy"
[ "$happy" = "$(tr -d '\n' <"$FIXTURES/happy/plan.json")" ] || fail "canonical bytes"
if python3 "$CHECKER" "$FIXTURES/invalid/plan.json" >/dev/null 2>"$ROOT_DIR/.seen/core-rel-003-invalid.err"; then fail "CORE-REL-003_invalid"; fi
grep -Fq core.rel.003.invalid "$ROOT_DIR/.seen/core-rel-003-invalid.err" || fail "invalid code"
if python3 "$CHECKER" "$FIXTURES/limit/plan.json" --max-bytes 32 >/dev/null 2>"$ROOT_DIR/.seen/core-rel-003-limit.err"; then fail "CORE-REL-003_limit"; fi
grep -Fq core.rel.003.limit "$ROOT_DIR/.seen/core-rel-003-limit.err" || fail "limit code"
status=0; python3 "$CHECKER" "$FIXTURES/cancel/plan.json" --test-cancel-after-read >/dev/null 2>"$ROOT_DIR/.seen/core-rel-003-cancel.err" || status=$?
[ "$status" -eq 130 ] || fail "CORE-REL-003_cancel"
python3 "$CHECKER" "$FIXTURES/happy/plan.json" --fuzz-seconds "${SEEN_CORE_REL_003_FUZZ_SECONDS:-1}" --seed 1101 >/dev/null 2>"$ROOT_DIR/.seen/core-rel-003-fuzz.err" || fail "fuzz"
grep -Fq seed=1101 "$ROOT_DIR/.seen/core-rel-003-fuzz.err" || fail "fuzz seed"
python3 "$BENCHMARK" "$FIXTURES/happy/plan.json" "$FIXTURES/happy/benchmark.json" | grep -Fq 'warmups=5 samples=30 status=pass' || fail "benchmark"
for symbol in ReleaseOptimizationPolicy ReleaseOptimizationOutcome validateReleaseOptimization releaseOptimizationValidateChecked renderReleaseOptimizationPlan; do grep -Fq "$symbol" "$SOURCE" || fail "native API $symbol"; done
for code in core.rel.003.invalid core.rel.003.limit core.rel.003.cancelled core.rel.003.platform; do grep -Fq "$code" "$SOURCE" || fail "native code $code"; done
for name in CORE-REL-003_happy CORE-REL-003_invalid CORE-REL-003_limit CORE-REL-003_cancel CORE-REL-003_cleanup; do grep -Fq "$name" "$ROOT_DIR/compiler_seen/tests/release/release_optimization.seen" || fail "native case $name"; done
grep -Fq -- '--lto=<mode>' "$COMPILER" || fail "LTO CLI"
! grep -Fq -- '--no-merged-release-lto' "$COMPILER" || fail "deprecated LTO bridge"
! grep -Fq 'using raw profile' "$COMPILER" || fail "raw profile fallback"
! grep -Fq '"release lto mode", "fallback"' "$COMPILER" || fail "LTO fallback"
grep -Fq 'hashString(loadFileContent(pgoPath))' "$COMPILER" || fail "profile cache identity"
grep -Fq 'CORE-REL-003 release acceptance' "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" || fail "Stage-1 cycle"
! grep -Fq '/tmp/' "$ROOT_DIR/scripts/pgo_build.sh" || fail "host temporary path"
grep -Fq 'seen-release-optimization-plan-v1' "$ROOT_DIR/schemas/compatibility-manifest.schema.json" || fail "compatibility binding"
grep -Fq '### PGO and explicit LTO modes' "$ROOT_DIR/docs/compiler-architecture.md" || fail "docs"
grep -Fq 'seen-release-optimization-plan-v1' "$ROOT_DIR/CHANGELOG.md" || fail "changelog"
python3 "$ROOT_DIR/scripts/check_native_boundaries.py" "$ROOT_DIR/docs/architecture/native-boundaries.json" >/dev/null || fail "ledger"
[ -z "$(jobs -pr)" ] || fail "CORE-REL-003_cleanup"
echo "PASS: PGO full-LTO and ThinLTO release contract"
