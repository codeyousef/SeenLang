#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
FIXTURES="$ROOT_DIR/tests/fixtures/core-rel-002"
CHECKER="$ROOT_DIR/scripts/check_build_instrumentation.py"
BENCHMARK="$ROOT_DIR/scripts/benchmark_build_instrumentation.py"
SOURCE="$ROOT_DIR/compiler_seen/src/release/build_instrumentation.seen"
COMPILER="$ROOT_DIR/compiler_seen/src/main_compiler.seen"
NATIVE_TEST="$ROOT_DIR/compiler_seen/tests/release/build_instrumentation.seen"
EXAMPLE="$ROOT_DIR/compiler_seen/examples/build_instrumentation.seen"

fail() { echo "FAIL: build instrumentation contract: $*" >&2; exit 1; }

python3 -m py_compile "$CHECKER" "$BENCHMARK" \
    "$ROOT_DIR/tests/misc_root_tests/seen_build_instrumentation_unit.py" ||
    fail "Python syntax"
python3 "$ROOT_DIR/tests/misc_root_tests/seen_build_instrumentation_unit.py" \
    >/dev/null || fail "unit matrix"

happy=$(python3 "$CHECKER" --evidence "$FIXTURES/happy/evidence.json") ||
    fail "CORE-REL-002_happy"
[ "$happy" = "$(tr -d '\n' <"$FIXTURES/happy/evidence.json")" ] ||
    fail "happy evidence is not canonical"

if python3 "$CHECKER" --evidence "$FIXTURES/invalid/evidence.json" \
    >/dev/null 2>"$ROOT_DIR/.seen/core-rel-002-invalid.err"; then
    fail "CORE-REL-002_invalid was accepted"
fi
grep -Fq core.rel.002.invalid "$ROOT_DIR/.seen/core-rel-002-invalid.err" ||
    fail "invalid code"
if python3 "$CHECKER" --evidence "$FIXTURES/limit/evidence.json" \
    --max-bytes 32 >/dev/null 2>"$ROOT_DIR/.seen/core-rel-002-limit.err"; then
    fail "CORE-REL-002_limit was accepted"
fi
grep -Fq core.rel.002.limit "$ROOT_DIR/.seen/core-rel-002-limit.err" ||
    fail "limit code"
cancel_status=0
python3 "$CHECKER" --evidence "$FIXTURES/cancel/evidence.json" \
    --test-cancel-after-read >/dev/null 2>"$ROOT_DIR/.seen/core-rel-002-cancel.err" ||
    cancel_status=$?
[ "$cancel_status" -eq 130 ] || fail "CORE-REL-002_cancel status"

python3 "$CHECKER" --evidence "$FIXTURES/happy/evidence.json" \
    --fuzz-seconds "${SEEN_CORE_REL_002_FUZZ_SECONDS:-1}" --seed 1101 \
    >/dev/null 2>"$ROOT_DIR/.seen/core-rel-002-fuzz.err" || fail "seed-1101 fuzz"
grep -Fq seed=1101 "$ROOT_DIR/.seen/core-rel-002-fuzz.err" || fail "fuzz evidence"
python3 "$BENCHMARK" "$FIXTURES/happy/evidence.json" \
    "$FIXTURES/happy/benchmark.json" | grep -Fq \
    'warmups=5 samples=30 status=pass' || fail "5/30/5 benchmark"

for symbol in BuildInstrumentationPolicy BuildInstrumentationEvidence \
    validateBuildInstrumentation buildInstrumentationValidateChecked \
    compileOnlyBuildInstrumentationEvidence \
    renderBuildInstrumentationEvidence; do
    grep -Fq "$symbol" "$SOURCE" || fail "native API omitted $symbol"
done
for code in core.rel.002.invalid core.rel.002.limit core.rel.002.cancelled \
    core.rel.002.platform; do
    grep -Fq "$code" "$SOURCE" || fail "native API omitted $code"
done
for case_name in CORE-REL-002_happy CORE-REL-002_invalid \
    CORE-REL-002_limit CORE-REL-002_cancel CORE-REL-002_cleanup; do
    grep -Fq "$case_name" "$NATIVE_TEST" || fail "native test omitted $case_name"
done
grep -Fq validateBuildInstrumentation "$EXAMPLE" || fail "example"
for flag in --debug --coverage --sanitize --instrumentation-report; do
    grep -Fq -- "$flag" "$COMPILER" || fail "CLI omitted $flag"
done
grep -Fq 'generator.setSanitizerMode(sanitizePolicy)' "$COMPILER" ||
    fail "sanitizer was not wired into Seen codegen"
grep -Fq 'sanitizerClangFlags + " -c -I "' "$COMPILER" ||
    fail "retained native objects omit instrumentation"
grep -Fq 'renderBuildInstrumentationEvidence(evidence)' "$COMPILER" ||
    fail "compiler omits canonical evidence emission"
grep -Fq 'compiler_seen/tests/release/build_instrumentation.seen' \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" || fail "Stage-1 native test"
grep -Fq 'instrumentation-report' "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" ||
    fail "Stage-1 CLI integration"
grep -Fq 'seen-build-instrumentation-evidence-v1' \
    "$ROOT_DIR/schemas/compatibility-manifest.schema.json" || fail "schema binding"
grep -Fq '### Debug, coverage, and sanitizer builds' \
    "$ROOT_DIR/docs/compiler-architecture.md" || fail "architecture docs"
grep -Fq 'seen-build-instrumentation-evidence-v1' "$ROOT_DIR/CHANGELOG.md" ||
    fail "changelog"
python3 "$ROOT_DIR/scripts/check_native_boundaries.py" \
    "$ROOT_DIR/docs/architecture/native-boundaries.json" >/dev/null || fail "ledger"
if grep -Eq '^[[:space:]]*extern[[:space:]]+fun' "$SOURCE"; then
    fail "unreviewed FFI"
fi
[ -z "$(jobs -pr)" ] || fail "CORE-REL-002_cleanup leaked a child"
echo "PASS: debug coverage and sanitizer build contract"
