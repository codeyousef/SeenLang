#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CHECK="$ROOT_DIR/scripts/check_test_migration.py"
FIXTURES="$ROOT_DIR/tests/fixtures/test-001f"
NATIVE="$ROOT_DIR/compiler_seen/src/testing/migration.seen"
NATIVE_TEST="$ROOT_DIR/compiler_seen/tests/test_001f_migration.seen"
fail() { echo "FAIL: TEST-001F migration contract: $*" >&2; exit 1; }

python3 -m py_compile "$CHECK" "$ROOT_DIR/tests/runner/test_migration_unit.py" || fail "Python syntax"
python3 "$ROOT_DIR/tests/runner/test_migration_unit.py" >/dev/null || fail "unit matrix"
python3 "$CHECK" --validate "$FIXTURES/happy/plan.json" >/dev/null || fail "TEST-001F_happy"
if python3 "$CHECK" --validate "$FIXTURES/invalid/plan.json" >/dev/null 2>"$ROOT_DIR/.seen/test-001f-invalid.err"; then fail "TEST-001F_invalid"; fi
grep -Fq test.001f.invalid "$ROOT_DIR/.seen/test-001f-invalid.err" || fail "invalid diagnostic"
if python3 "$CHECK" --validate "$FIXTURES/limit/plan.json" --max-tests 2 >/dev/null 2>"$ROOT_DIR/.seen/test-001f-limit.err"; then fail "TEST-001F_limit"; fi
grep -Fq test.001f.limit "$ROOT_DIR/.seen/test-001f-limit.err" || fail "limit diagnostic"
status=0
python3 "$CHECK" --validate "$FIXTURES/cancel/plan.json" --test-cancel-after-read >/dev/null 2>"$ROOT_DIR/.seen/test-001f-cancel.err" || status=$?
[ "$status" -eq 130 ] || fail "TEST-001F_cancel"

first="$ROOT_DIR/.seen/test-001f-first.json"; second="$ROOT_DIR/.seen/test-001f-second.json"
python3 "$CHECK" --discover "$ROOT_DIR" >"$first" || fail "repository migration discovery"
python3 "$CHECK" --discover "$ROOT_DIR" >"$second" || fail "repository migration rediscovery"
cmp -s "$first" "$second" || fail "migration discovery changed"
python3 "$CHECK" --validate "$first" >/dev/null || fail "repository migration validation"
python3 "$CHECK" --validate "$FIXTURES/happy/plan.json" --fuzz-seconds "${SEEN_TEST_001F_FUZZ_SECONDS:-1}" --seed 1101 >/dev/null 2>"$ROOT_DIR/.seen/test-001f-fuzz.err" || fail "fuzz"
grep -Fq seed=1101 "$ROOT_DIR/.seen/test-001f-fuzz.err" || fail "fuzz seed"
python3 "$CHECK" --validate "$FIXTURES/happy/plan.json" --benchmark-limit-ms 10 | grep -Fq 'warmups=5 samples=30' || fail "benchmark"

for symbol in TestMigrationSource TestMigrationEntry TestMigrationPlan TestMigrationOutcome buildTestMigrationPlan buildTestMigrationPlanChecked renderTestMigrationPlan testMigrationWorkingRoot; do
    grep -Fq "$symbol" "$NATIVE" || fail "native API $symbol"
done
for code in test.001f.invalid test.001f.limit test.001f.cancelled; do grep -Fq "$code" "$NATIVE" || fail "native diagnostic $code"; done
for name in TEST-001F_happy TEST-001F_invalid TEST-001F_limit TEST-001F_cancel TEST-001F_cleanup; do grep -Fq "$name" "$NATIVE_TEST" || fail "native case $name"; done
grep -Fq 'tests/fixtures/external_package/tests' "$ROOT_DIR/scripts/discover_seen_tests.py" || fail "external discovery"
grep -Fq 'testMigrationWorkingRoot(relative)' "$ROOT_DIR/compiler_seen/src/testing/cli_runner.seen" || fail "package working root"
grep -Fq 'compiler_seen/tests/test_001f_migration.seen' "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" || fail "Stage-1 wiring"
grep -Fq -- '--discover "$REPO_ROOT"' "$ROOT_DIR/scripts/run_all_tests.sh" || fail "legacy migration discovery"
grep -Fq 'seen-test-migration-v1' "$ROOT_DIR/schemas/compatibility-manifest.schema.json" || fail "compatibility binding"
grep -Fq 'Package-owned test migration' "$ROOT_DIR/docs/testing.md" || fail "documentation"
grep -Fq 'seen-test-migration-v1' "$ROOT_DIR/CHANGELOG.md" || fail "changelog"
grep -Fq 'PASS: TEST-001F_external_package' "$ROOT_DIR/tests/fixtures/external_package/tests/layout_contract.seen" || fail "external runnable"
[ -z "$(jobs -pr)" ] || fail "TEST-001F_cleanup"
echo "PASS: compiler, stdlib, and external-package test migration contract"
