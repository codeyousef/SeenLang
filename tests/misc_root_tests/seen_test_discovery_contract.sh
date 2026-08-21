#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
FIXTURES="$ROOT_DIR/tests/fixtures/test-001a"
DISCOVERY="$ROOT_DIR/scripts/discover_seen_tests.py"
BENCHMARK="$ROOT_DIR/scripts/benchmark_test_discovery.py"
NATIVE="$ROOT_DIR/compiler_seen/src/testing/discovery.seen"
NATIVE_TEST="$ROOT_DIR/compiler_seen/tests/test_discovery.seen"
CATEGORIES="$ROOT_DIR/seen_std/src/testing/categories.seen"
fail() { echo "FAIL: test discovery contract: $*" >&2; exit 1; }

python3 -m py_compile "$DISCOVERY" "$BENCHMARK" \
    "$ROOT_DIR/tests/runner/test_discovery_unit.py" || fail "Python syntax"
python3 "$ROOT_DIR/tests/runner/test_discovery_unit.py" >/dev/null ||
    fail "unit matrix"

actual=$(python3 "$DISCOVERY" --discover "$FIXTURES/happy") ||
    fail "TEST-001A_happy"
[ "$actual" = "$(tr -d '\n' <"$FIXTURES/happy/expected.json")" ] ||
    fail "deterministic canonical bytes"
python3 "$DISCOVERY" --validate "$FIXTURES/happy/expected.json" >/dev/null ||
    fail "happy manifest validation"

if python3 "$DISCOVERY" --validate "$FIXTURES/invalid/manifest.json" \
    >/dev/null 2>"$ROOT_DIR/.seen/test-001a-invalid.err"; then
    fail "TEST-001A_invalid"
fi
grep -Fq test.001a.invalid "$ROOT_DIR/.seen/test-001a-invalid.err" ||
    fail "invalid diagnostic"

if python3 "$DISCOVERY" --validate "$FIXTURES/limit/manifest.json" \
    --max-tests 0 >/dev/null 2>"$ROOT_DIR/.seen/test-001a-limit.err"; then
    fail "TEST-001A_limit"
fi
grep -Fq test.001a.limit "$ROOT_DIR/.seen/test-001a-limit.err" ||
    fail "limit diagnostic"

status=0
python3 "$DISCOVERY" --validate "$FIXTURES/cancel/manifest.json" \
    --test-cancel-after-read >/dev/null \
    2>"$ROOT_DIR/.seen/test-001a-cancel.err" || status=$?
[ "$status" -eq 130 ] || fail "TEST-001A_cancel"
grep -Fq test.001a.cancelled "$ROOT_DIR/.seen/test-001a-cancel.err" ||
    fail "cancel diagnostic"

first="$ROOT_DIR/.seen/test-001a-repo-first.json"
second="$ROOT_DIR/.seen/test-001a-repo-second.json"
python3 "$DISCOVERY" --discover "$ROOT_DIR" >"$first" ||
    fail "repository discovery"
python3 "$DISCOVERY" --discover "$ROOT_DIR" >"$second" ||
    fail "repository rediscovery"
cmp -s "$first" "$second" || fail "repository discovery changed"
python3 "$DISCOVERY" --validate "$first" >/dev/null ||
    fail "repository manifest validation"

python3 "$DISCOVERY" --validate "$FIXTURES/happy/expected.json" \
    --fuzz-seconds "${SEEN_TEST_001A_FUZZ_SECONDS:-1}" --seed 1101 \
    >/dev/null 2>"$ROOT_DIR/.seen/test-001a-fuzz.err" || fail "fuzz"
grep -Fq seed=1101 "$ROOT_DIR/.seen/test-001a-fuzz.err" ||
    fail "fuzz seed"
python3 "$BENCHMARK" "$FIXTURES/happy/expected.json" \
    "$FIXTURES/happy/benchmark.json" |
    grep -Fq 'warmups=5 samples=30 status=pass' || fail "benchmark"

for symbol in TestDiscoveryCandidate TestDiscoveryManifest \
    TestDiscoveryOutcome discoverTests discoverTestsChecked \
    renderTestDiscoveryManifest; do
    grep -Fq "$symbol" "$NATIVE" || fail "native API $symbol"
done
for symbol in TestPrimaryCategory TestPlatformCategory \
    testPrimaryCategoryName testPlatformCategoryName; do
    grep -Fq "$symbol" "$CATEGORIES" || fail "standard category API $symbol"
done
for code in test.001a.invalid test.001a.limit test.001a.cancelled; do
    grep -Fq "$code" "$NATIVE" || fail "native diagnostic $code"
done
for name in TEST-001A_happy TEST-001A_invalid TEST-001A_limit \
    TEST-001A_cancel TEST-001A_cleanup; do
    grep -Fq "$name" "$NATIVE_TEST" || fail "native case $name"
done
grep -Fq 'compiler_seen/tests/test_discovery.seen' \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" || fail "Stage-1 wiring"
grep -Fq -- '--discover "$REPO_ROOT"' "$ROOT_DIR/scripts/run_all_tests.sh" ||
    fail "legacy inventory wiring"
grep -Fq -- '--validate "$test_discovery_manifest"' \
    "$ROOT_DIR/scripts/run_all_tests.sh" || fail "legacy inventory validation"
grep -Fq 'seen-test-discovery-v1' \
    "$ROOT_DIR/schemas/compatibility-manifest.schema.json" ||
    fail "compatibility binding"
grep -Fq 'modulePath == "testing.categories"' \
    "$ROOT_DIR/compiler_seen/src/main_compiler.seen" ||
    fail "shared standard category resolution"
grep -Fq 'Deterministic test discovery' "$ROOT_DIR/docs/testing.md" ||
    fail "documentation"
grep -Fq 'seen-test-discovery-v1' "$ROOT_DIR/CHANGELOG.md" ||
    fail "changelog"
[ -z "$(jobs -pr)" ] || fail "TEST-001A_cleanup"
echo "PASS: deterministic test discovery and categories contract"
