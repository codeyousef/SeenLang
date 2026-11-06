#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
FIXTURES="$ROOT_DIR/tests/fixtures/core-003c"
CHECKER="$ROOT_DIR/scripts/check_production_ir_policy.py"
BENCHMARK="$ROOT_DIR/scripts/benchmark_production_ir_policy.py"
LEDGER="$ROOT_DIR/docs/architecture/native-boundaries.json"
ARTIFACT_HELPER="$ROOT_DIR/scripts/artifact_root.sh"

fail() {
    echo "FAIL: production IR policy contract: $*" >&2
    exit 1
}

# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_HELPER" || fail "could not load artifact-root helper"
seen_artifact_root_init "$ROOT_DIR" || fail "could not initialize artifact root"
test_scope=$(seen_artifact_scope_init production-ir-policy-contract-tests) ||
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

python3 "$ROOT_DIR/tests/misc_root_tests/seen_production_ir_policy_unit.py" \
    >/dev/null || fail "unit and branch matrix"
python3 "$CHECKER" "$FIXTURES/happy/policy.json" \
    >"$TEST_ROOT/happy-a.json" || fail "CORE-003C_happy"
python3 "$CHECKER" "$FIXTURES/happy/policy.json" \
    >"$TEST_ROOT/happy-b.json" || fail "CORE-003C_happy repeat"
cmp -s "$TEST_ROOT/happy-a.json" "$TEST_ROOT/happy-b.json" ||
    fail "CORE-003C_happy was nondeterministic"
cmp -s "$TEST_ROOT/happy-a.json" "$FIXTURES/happy/expected.json" ||
    fail "CORE-003C_happy bytes changed"

if python3 "$CHECKER" "$FIXTURES/invalid/policy.json" \
    >"$TEST_ROOT/invalid.json" 2>"$TEST_ROOT/invalid.err"; then
    fail "CORE-003C_invalid was accepted"
fi
grep -Fq 'core.003c.invalid' "$TEST_ROOT/invalid.err" ||
    fail "CORE-003C_invalid omitted its typed diagnostic"

if python3 "$CHECKER" "$FIXTURES/limit/policy.json" \
    >"$TEST_ROOT/limit.json" 2>"$TEST_ROOT/limit.err"; then
    fail "CORE-003C_limit was accepted"
fi
grep -Fq 'core.003c.limit' "$TEST_ROOT/limit.err" ||
    fail "CORE-003C_limit omitted its typed diagnostic"

cancel_status=0
SEEN_PRODUCTION_IR_TEST_HOOKS=1 python3 "$CHECKER" \
    "$FIXTURES/cancel/policy.json" --test-cancel-after-read \
    >"$TEST_ROOT/cancel.json" 2>"$TEST_ROOT/cancel.err" || cancel_status=$?
[ "$cancel_status" -eq 130 ] || fail "CORE-003C_cancel returned $cancel_status"
[ ! -s "$TEST_ROOT/cancel.json" ] || fail "CORE-003C_cancel emitted partial output"
grep -Fq 'core.003c.cancelled' "$TEST_ROOT/cancel.err" ||
    fail "CORE-003C_cancel omitted its typed diagnostic"

python3 "$CHECKER" "$FIXTURES/happy/policy.json" \
    --fuzz-seconds "${SEEN_CORE_003C_FUZZ_SECONDS:-1}" --seed 1101 \
    >"$TEST_ROOT/fuzz.json" 2>"$TEST_ROOT/fuzz.err" ||
    fail "seed-1101 production-IR fuzz"
cmp -s "$TEST_ROOT/fuzz.json" "$FIXTURES/happy/expected.json" ||
    fail "fuzz changed canonical output"
grep -Fq 'seed=1101' "$TEST_ROOT/fuzz.err" || fail "fuzz omitted seed evidence"

python3 "$BENCHMARK" "$FIXTURES/happy/policy.json" \
    "$FIXTURES/happy/benchmark.json" >"$TEST_ROOT/benchmark.log" ||
    fail "5-warmup/30-sample benchmark"
grep -Fq 'warmups=5 samples=30 status=pass' "$TEST_ROOT/benchmark.log" ||
    fail "benchmark omitted policy evidence"

python3 "$ROOT_DIR/scripts/check_native_boundaries.py" "$LEDGER" >/dev/null ||
    fail "native-boundary ledger"
python3 -c 'import json,sys; expected=json.load(open(sys.argv[1], encoding="utf-8")); assert expected == {"children": 0, "descriptors": 0, "tasks": 0, "temporary_files": []}' \
    "$FIXTURES/cleanup/expected.json" || fail "cleanup expectation is invalid"
[ -z "$(jobs -pr)" ] || fail "CORE-003C_cleanup leaked a child"
if find "$FIXTURES" -type f \( -name '*.tmp' -o -name '.*.tmp' \) \
    -print -quit | grep -q .; then
    fail "CORE-003C_cleanup found a temporary artifact"
fi

echo "PASS: unmodified production IR policy contract"
