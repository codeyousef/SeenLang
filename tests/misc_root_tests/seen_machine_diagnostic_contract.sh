#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
FIXTURES="$ROOT_DIR/tests/fixtures/core-rel-001"
CHECKER="$ROOT_DIR/scripts/check_machine_diagnostic.py"
BENCHMARK="$ROOT_DIR/scripts/benchmark_machine_diagnostic.py"
SOURCE="$ROOT_DIR/compiler_seen/src/release/diagnostic_schema.seen"
NATIVE_TEST="$ROOT_DIR/compiler_seen/tests/release/machine_diagnostic.seen"
EXAMPLE="$ROOT_DIR/compiler_seen/examples/machine_diagnostic.seen"
STAGE1="$ROOT_DIR/scripts/seen_stage1_acceptance.sh"
ARCHITECTURE="$ROOT_DIR/docs/compiler-architecture.md"
CHANGELOG="$ROOT_DIR/CHANGELOG.md"
COMPATIBILITY_SCHEMA="$ROOT_DIR/schemas/compatibility-manifest.schema.json"
COMPATIBILITY_MANIFEST="$ROOT_DIR/releases/compatibility-manifest.json"
LEDGER="$ROOT_DIR/docs/architecture/native-boundaries.json"
ARTIFACT_HELPER="$ROOT_DIR/scripts/artifact_root.sh"

fail() {
    echo "FAIL: machine diagnostic contract: $*" >&2
    exit 1
}

# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_HELPER" || fail "could not load artifact-root helper"
seen_artifact_root_init "$ROOT_DIR" || fail "could not initialize artifact root"
test_scope=$(seen_artifact_scope_init machine-diagnostic-contract-tests) ||
    fail "could not initialize test scope"
TEST_ROOT=$(seen_artifact_mktemp_dir "$test_scope" run) ||
    fail "could not create test root"

cleanup() {
    local status=$?
    case "$TEST_ROOT" in
        "$test_scope"/run.*)
            [ -d "$TEST_ROOT" ] && [ ! -L "$TEST_ROOT" ] &&
                [ "${TEST_ROOT%/*}" = "$test_scope" ] || return 1
            rm -rf -- "$TEST_ROOT" || return 1
            ;;
        *) return 1 ;;
    esac
    return "$status"
}
trap cleanup EXIT

python3 -m py_compile "$CHECKER" "$BENCHMARK" \
    "$ROOT_DIR/tests/misc_root_tests/seen_machine_diagnostic_unit.py" ||
    fail "Python syntax"
python3 "$ROOT_DIR/tests/misc_root_tests/seen_machine_diagnostic_unit.py" \
    >/dev/null || fail "unit and branch matrix"

python3 "$CHECKER" "$FIXTURES/happy/diagnostic.json" \
    >"$TEST_ROOT/happy-a.json" || fail "CORE-REL-001_happy"
python3 "$CHECKER" "$FIXTURES/happy/diagnostic.json" \
    >"$TEST_ROOT/happy-b.json" || fail "CORE-REL-001_happy repeat"
cmp -s "$TEST_ROOT/happy-a.json" "$TEST_ROOT/happy-b.json" ||
    fail "CORE-REL-001_happy was nondeterministic"
cmp -s "$TEST_ROOT/happy-a.json" "$FIXTURES/happy/expected.json" ||
    fail "CORE-REL-001_happy bytes changed"

if python3 "$CHECKER" "$FIXTURES/invalid/diagnostic.json" \
    >"$TEST_ROOT/invalid.json" 2>"$TEST_ROOT/invalid.err"; then
    fail "CORE-REL-001_invalid was accepted"
fi
grep -Fq 'core.rel.001.invalid' "$TEST_ROOT/invalid.err" ||
    fail "CORE-REL-001_invalid omitted its typed code"

if python3 "$CHECKER" "$FIXTURES/limit/diagnostic.json" \
    >"$TEST_ROOT/limit.json" 2>"$TEST_ROOT/limit.err"; then
    fail "CORE-REL-001_limit was accepted"
fi
grep -Fq 'core.rel.001.limit' "$TEST_ROOT/limit.err" ||
    fail "CORE-REL-001_limit omitted its typed code"

cancel_status=0
SEEN_MACHINE_DIAGNOSTIC_TEST_HOOKS=1 python3 "$CHECKER" \
    "$FIXTURES/cancel/diagnostic.json" --test-cancel-after-read \
    >"$TEST_ROOT/cancel.json" 2>"$TEST_ROOT/cancel.err" || cancel_status=$?
[ "$cancel_status" -eq 130 ] ||
    fail "CORE-REL-001_cancel returned $cancel_status"
[ ! -s "$TEST_ROOT/cancel.json" ] ||
    fail "CORE-REL-001_cancel emitted partial output"
grep -Fq 'core.rel.001.cancelled' "$TEST_ROOT/cancel.err" ||
    fail "CORE-REL-001_cancel omitted its typed code"

python3 "$CHECKER" "$FIXTURES/happy/diagnostic.json" \
    --fuzz-seconds "${SEEN_CORE_REL_001_FUZZ_SECONDS:-1}" --seed 1101 \
    >"$TEST_ROOT/fuzz.json" 2>"$TEST_ROOT/fuzz.err" ||
    fail "seed-1101 diagnostic fuzz"
cmp -s "$TEST_ROOT/fuzz.json" "$FIXTURES/happy/expected.json" ||
    fail "fuzz changed canonical output"
grep -Fq 'seed=1101' "$TEST_ROOT/fuzz.err" ||
    fail "fuzz omitted seed evidence"

python3 "$BENCHMARK" "$FIXTURES/happy/diagnostic.json" \
    "$FIXTURES/happy/benchmark.json" >"$TEST_ROOT/benchmark.log" ||
    fail "5-warmup/30-sample benchmark"
grep -Fq 'warmups=5 samples=30 status=pass' "$TEST_ROOT/benchmark.log" ||
    fail "benchmark omitted policy evidence"

for symbol in BackendMaturity MachineDiagnosticContext MachineDiagnostic \
    validateMachineDiagnostic renderMachineDiagnostic coreRel001Error; do
    grep -Fq "$symbol" "$SOURCE" || fail "native API omitted $symbol"
done
for code in core.rel.001.invalid core.rel.001.limit core.rel.001.cancelled \
    core.rel.001.platform; do
    grep -Fq "$code" "$SOURCE" || fail "native API omitted $code"
done
for maturity in unsupported compile-only experimental-hardware verified \
    production-certified; do
    grep -Fq "$maturity" "$SOURCE" || fail "maturity omitted $maturity"
done
for case_name in CORE-REL-001_happy CORE-REL-001_invalid \
    CORE-REL-001_limit CORE-REL-001_cancel CORE-REL-001_cleanup; do
    grep -Fq "$case_name" "$NATIVE_TEST" ||
        fail "native regression omitted $case_name"
done
grep -Fq 'validateMachineDiagnostic' "$EXAMPLE" ||
    fail "example omits the public Result entry point"
for fixture in compiler_seen/tests/release/machine_diagnostic.seen \
    compiler_seen/examples/machine_diagnostic.seen; do
    grep -Fq "$fixture" "$STAGE1" || fail "Stage-1 acceptance omits $fixture"
done

grep -Fq 'seen-machine-diagnostic-v1' "$COMPATIBILITY_SCHEMA" ||
    fail "compatibility schema omits diagnostic binding"
grep -Fq 'seen-object-cache-abi-v3' "$COMPATIBILITY_MANIFEST" ||
    fail "compatibility manifest omits bound compiler identity"
python3 "$ROOT_DIR/scripts/check_compatibility_manifest.py" \
    "$COMPATIBILITY_MANIFEST" >/dev/null || fail "compatibility manifest"
grep -Fq '### Stable machine diagnostics' "$ARCHITECTURE" ||
    fail "compiler architecture omits CORE-REL-001"
grep -Fq 'seen-machine-diagnostic-v1' "$CHANGELOG" ||
    fail "changelog omits CORE-REL-001"

python3 "$ROOT_DIR/scripts/check_native_boundaries.py" "$LEDGER" >/dev/null ||
    fail "native-boundary ledger"
if grep -Eq '^[[:space:]]*extern[[:space:]]+fun' "$SOURCE"; then
    fail "machine diagnostic schema introduced unreviewed FFI"
fi
python3 -c 'import json,sys; value=json.load(open(sys.argv[1], encoding="utf-8")); assert value == {"children": 0, "descriptors": 0, "tasks": 0, "temporary_files": []}' \
    "$FIXTURES/cleanup/expected.json" || fail "cleanup expectation"
[ -z "$(jobs -pr)" ] || fail "CORE-REL-001_cleanup leaked a child"
if find "$FIXTURES" -type f \( -name '*.tmp' -o -name '.*.tmp' \) \
    -print -quit | grep -q .; then
    fail "CORE-REL-001_cleanup found a temporary artifact"
fi

echo "PASS: stable machine diagnostic contract"
