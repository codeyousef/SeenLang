#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CHECKER="$ROOT_DIR/scripts/check_test_snapshots.py"
HAPPY="$ROOT_DIR/tests/fixtures/test-001c/happy/snapshot.json"
INVALID="$ROOT_DIR/tests/fixtures/test-001c/invalid/snapshot.json"
ASSERTIONS="$ROOT_DIR/compiler_seen/src/testing/assertions.seen"
SNAPSHOT="$ROOT_DIR/compiler_seen/src/testing/snapshot.seen"
fail() { echo "FAIL: assertion and snapshot contract: $*" >&2; exit 1; }

python3 -m py_compile "$CHECKER" \
    "$ROOT_DIR/tests/runner/test_snapshots_unit.py" || fail "Python syntax"
python3 "$ROOT_DIR/tests/runner/test_snapshots_unit.py" >/dev/null ||
    fail "unit matrix"
python3 "$CHECKER" --validate "$HAPPY" >/dev/null || fail "happy snapshot"
if python3 "$CHECKER" --validate "$INVALID" >/dev/null \
    2>"$ROOT_DIR/.seen/test-001c-invalid.err"; then
    fail "invalid snapshot accepted"
fi
grep -Fq test.001c.invalid "$ROOT_DIR/.seen/test-001c-invalid.err" ||
    fail "stable invalid diagnostic"
python3 "$CHECKER" --validate "$HAPPY" \
    --fuzz-seconds "${SEEN_TEST_001C_FUZZ_SECONDS:-1}" --seed 1101 \
    >/dev/null 2>"$ROOT_DIR/.seen/test-001c-fuzz.err" || fail "fuzz"
grep -Fq seed=1101 "$ROOT_DIR/.seen/test-001c-fuzz.err" || fail "fuzz seed"
python3 "$CHECKER" --validate "$HAPPY" --benchmark-limit-ms 10 |
    grep -Fq 'warmups=5 samples=30' || fail "benchmark"

for symbol in AssertionOperator AssertionRequest AssertionResult \
    evaluateAssertion evaluateAssertionChecked renderAssertionResult; do
    grep -Fq "$symbol" "$ASSERTIONS" || fail "native assertion API $symbol"
done
for symbol in SnapshotUpdatePolicy SnapshotRequest SnapshotResult \
    compareSnapshot compareSnapshotChecked snapshotPath snapshotWriteContent; do
    grep -Fq "$symbol" "$SNAPSHOT" || fail "native snapshot API $symbol"
done
for code in test.001c.invalid test.001c.limit test.001c.cancelled; do
    grep -Fq "$code" "$ASSERTIONS" || fail "native diagnostic $code"
done
for name in TEST-001C_happy TEST-001C_invalid TEST-001C_limit \
    TEST-001C_cancel TEST-001C_cleanup; do
    grep -Fq "$name" "$ROOT_DIR/compiler_seen/tests/test_001c_assertions.seen" ||
        fail "native case $name"
done
grep -Fq 'seen-test-assertion-v1' "$ROOT_DIR/docs/testing.md" ||
    fail "assertion documentation"
grep -Fq 'seen-test-snapshot-v1' "$ROOT_DIR/docs/testing.md" ||
    fail "snapshot documentation"
grep -Fq 'seen-test-snapshot-v1' "$ROOT_DIR/CHANGELOG.md" || fail "changelog"
grep -Fq 'seen-test-snapshot-v1' \
    "$ROOT_DIR/schemas/compatibility-manifest.schema.json" ||
    fail "compatibility binding"
[ -z "$(jobs -pr)" ] || fail "TEST-001C_cleanup"
echo "PASS: native Seen assertions and exact snapshot contract"
