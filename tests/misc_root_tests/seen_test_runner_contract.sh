#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CHECKER="$ROOT_DIR/scripts/check_test_runner.py"
HAPPY="$ROOT_DIR/tests/fixtures/test-001b/happy/report.json"
INVALID="$ROOT_DIR/tests/fixtures/test-001b/invalid/report.json"
NATIVE="$ROOT_DIR/compiler_seen/src/testing/runner.seen"
CLI="$ROOT_DIR/compiler_seen/src/testing/cli_runner.seen"
fail() { echo "FAIL: canonical test runner contract: $*" >&2; exit 1; }

python3 -m py_compile "$CHECKER" "$ROOT_DIR/tests/runner/test_runner_unit.py" ||
    fail "Python syntax"
python3 "$ROOT_DIR/tests/runner/test_runner_unit.py" >/dev/null ||
    fail "unit matrix"
python3 "$CHECKER" --validate "$HAPPY" >/dev/null || fail "happy report"
if python3 "$CHECKER" --validate "$INVALID" >/dev/null \
    2>"$ROOT_DIR/.seen/test-001b-invalid.err"; then
    fail "invalid report accepted"
fi
grep -Fq test.001b.invalid "$ROOT_DIR/.seen/test-001b-invalid.err" ||
    fail "stable invalid diagnostic"
python3 "$CHECKER" --validate "$HAPPY" \
    --fuzz-seconds "${SEEN_TEST_001B_FUZZ_SECONDS:-1}" --seed 1101 \
    >/dev/null 2>"$ROOT_DIR/.seen/test-001b-fuzz.err" || fail "fuzz"
grep -Fq seed=1101 "$ROOT_DIR/.seen/test-001b-fuzz.err" || fail "fuzz seed"
python3 "$CHECKER" --validate "$HAPPY" --benchmark-limit-ms 10 |
    grep -Fq 'warmups=5 samples=30' || fail "benchmark"

for symbol in TestRunnerOptions TestRunPlan TestExecutionResult TestRunReport \
    buildTestRunPlan finalizeTestRun renderTestRunReport renderTestRunJunit; do
    grep -Fq "$symbol" "$NATIVE" || fail "native API $symbol"
done
for code in test.001b.invalid test.001b.limit test.001b.cancelled; do
    grep -Fq "$code" "$NATIVE" || fail "native diagnostic $code"
done
for boundary in 'timeout --signal=TERM' 'SEEN_PACKAGE_CLIENT=' \
    'writeTextAtomically' 'testCliSafeRelativePath'; do
    grep -Fq "$boundary" "$CLI" || fail "execution boundary $boundary"
done
for name in TEST-001B_happy TEST-001B_invalid TEST-001B_limit \
    TEST-001B_cancel TEST-001B_cleanup; do
    grep -Fq "$name" "$ROOT_DIR/compiler_seen/tests/test_001b_runner.seen" ||
        fail "native case $name"
done
grep -Fq 'runCanonicalTests' "$ROOT_DIR/compiler_seen/src/main_compiler.seen" ||
    fail "shipped CLI wiring"
grep -Fq 'compiler_seen/target/seen-pkg' \
    "$ROOT_DIR/compiler_seen/src/main_compiler.seen" ||
    fail "source-checkout package-client resolution"
grep -Fq 'seen-test-run-v1' "$ROOT_DIR/docs/testing.md" || fail "documentation"
grep -Fq 'seen-test-run-v1' "$ROOT_DIR/CHANGELOG.md" || fail "changelog"
grep -Fq 'seen-test-run-v1' \
    "$ROOT_DIR/schemas/compatibility-manifest.schema.json" ||
    fail "compatibility binding"
[ -z "$(jobs -pr)" ] || fail "TEST-001B_cleanup"
echo "PASS: canonical Seen test runner and exit-code contract"
