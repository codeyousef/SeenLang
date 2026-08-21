#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CHECKER="$ROOT_DIR/scripts/check_test_fixture.py"
HAPPY="$ROOT_DIR/tests/fixtures/test-001d/happy/fixture.json"
INVALID="$ROOT_DIR/tests/fixtures/test-001d/invalid/fixture.json"
NATIVE="$ROOT_DIR/compiler_seen/src/testing/fixture.seen"
fail() { echo "FAIL: deterministic fixture contract: $*" >&2; exit 1; }

python3 -m py_compile "$CHECKER" \
    "$ROOT_DIR/tests/runner/test_fixture_unit.py" || fail "Python syntax"
python3 "$ROOT_DIR/tests/runner/test_fixture_unit.py" >/dev/null ||
    fail "unit matrix"
python3 "$CHECKER" --validate "$HAPPY" >/dev/null || fail "happy fixture"
if python3 "$CHECKER" --validate "$INVALID" >/dev/null \
    2>"$ROOT_DIR/.seen/test-001d-invalid.err"; then
    fail "invalid fixture accepted"
fi
grep -Fq test.001d.invalid "$ROOT_DIR/.seen/test-001d-invalid.err" ||
    fail "stable invalid diagnostic"
python3 "$CHECKER" --validate "$HAPPY" \
    --fuzz-seconds "${SEEN_TEST_001D_FUZZ_SECONDS:-1}" --seed 1101 \
    >/dev/null 2>"$ROOT_DIR/.seen/test-001d-fuzz.err" || fail "fuzz"
grep -Fq seed=1101 "$ROOT_DIR/.seen/test-001d-fuzz.err" || fail "fuzz seed"
python3 "$CHECKER" --validate "$HAPPY" --benchmark-limit-ms 10 |
    grep -Fq 'warmups=5 samples=30' || fail "benchmark"

for symbol in FixtureFile FixtureEnvironment FixtureRequest FixturePlan \
    planFixture planFixtureChecked fixtureCleanupPath renderFixturePlan; do
    grep -Fq "$symbol" "$NATIVE" || fail "native fixture API $symbol"
done
for code in test.001d.invalid test.001d.limit test.001d.cancelled \
    test.001d.platform; do
    grep -Fq "$code" "$NATIVE" || fail "native diagnostic $code"
done
for name in TEST-001D_happy TEST-001D_invalid TEST-001D_limit \
    TEST-001D_cancel TEST-001D_cleanup; do
    grep -Fq "$name" "$ROOT_DIR/compiler_seen/tests/test_001d_fixtures.seen" ||
        fail "native case $name"
done
grep -Fq 'seen-test-fixture-v1' "$ROOT_DIR/docs/testing.md" ||
    fail "documentation"
grep -Fq 'seen-test-fixture-v1' "$ROOT_DIR/CHANGELOG.md" || fail "changelog"
grep -Fq 'seen-test-fixture-v1' \
    "$ROOT_DIR/schemas/compatibility-manifest.schema.json" ||
    fail "compatibility binding"
[ -z "$(jobs -pr)" ] || fail "TEST-001D_cleanup"
echo "PASS: deterministic isolated fixture contract"
