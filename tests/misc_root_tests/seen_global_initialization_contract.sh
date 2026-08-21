#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
FIXTURES="$ROOT_DIR/tests/fixtures/core-003b"
CHECKER="$ROOT_DIR/scripts/check_global_initialization.py"
BENCHMARK="$ROOT_DIR/scripts/benchmark_global_initialization.py"
SOURCE="$ROOT_DIR/compiler_seen/src/imports/graph.seen"
DIAGNOSTICS="$ROOT_DIR/compiler_seen/src/release/diagnostic_schema.seen"
ENTRY="$ROOT_DIR/compiler_seen/src/main_compiler.seen"
MODULE_EMIT="$ROOT_DIR/compiler_seen/src/codegen/ir_module_emit.seen"
FEATURE_STATE="$ROOT_DIR/compiler_seen/src/codegen/ir_codegen_feature_state.seen"
TAIL_DRIVER="$ROOT_DIR/compiler_seen/src/codegen/ir_module_tail_driver.seen"
NATIVE_TEST="$ROOT_DIR/compiler_seen/tests/release/global_initialization.seen"
EXAMPLE="$ROOT_DIR/compiler_seen/examples/global_initialization_plan.seen"
STAGE1="$ROOT_DIR/scripts/seen_stage1_acceptance.sh"
COMPATIBILITY="$ROOT_DIR/releases/compatibility-manifest.json"
COMPATIBILITY_SCHEMA="$ROOT_DIR/schemas/compatibility-manifest.schema.json"
ARCHITECTURE="$ROOT_DIR/docs/compiler-architecture.md"
CHANGELOG="$ROOT_DIR/CHANGELOG.md"
LEDGER="$ROOT_DIR/docs/architecture/native-boundaries.json"
FROZEN_COMPATIBILITY="$ROOT_DIR/bootstrap/stage1_frozen.compatibility-manifest.json"
FROZEN_COMPATIBILITY_HASH="$ROOT_DIR/bootstrap/stage1_frozen.compatibility-manifest.sha256"
ARTIFACT_HELPER="$ROOT_DIR/scripts/artifact_root.sh"

fail() {
    echo "FAIL: global initialization contract: $*" >&2
    exit 1
}

# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_HELPER" || fail "could not load artifact-root helper"
seen_artifact_root_init "$ROOT_DIR" || fail "could not initialize artifact root"
test_scope=$(seen_artifact_scope_init global-initialization-contract-tests) ||
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

python3 "$ROOT_DIR/tests/misc_root_tests/seen_global_initialization_unit.py" \
    >/dev/null || fail "unit and branch matrix"
python3 "$CHECKER" "$FIXTURES/happy/plan.json" \
    >"$TEST_ROOT/happy-a.json" || fail "CORE-003B_happy"
python3 "$CHECKER" "$FIXTURES/happy/plan.json" \
    >"$TEST_ROOT/happy-b.json" || fail "CORE-003B_happy repeat"
cmp -s "$TEST_ROOT/happy-a.json" "$TEST_ROOT/happy-b.json" ||
    fail "CORE-003B_happy was nondeterministic"
cmp -s "$TEST_ROOT/happy-a.json" "$FIXTURES/happy/expected.json" ||
    fail "CORE-003B_happy bytes changed"

if python3 "$CHECKER" "$FIXTURES/invalid/plan.json" \
    >"$TEST_ROOT/invalid.json" 2>"$TEST_ROOT/invalid.err"; then
    fail "CORE-003B_invalid was accepted"
fi
grep -Fq 'core.003b.invalid' "$TEST_ROOT/invalid.err" ||
    fail "CORE-003B_invalid omitted its typed diagnostic"

if python3 "$CHECKER" "$FIXTURES/limit/plan.json" --max-modules 0 \
    >"$TEST_ROOT/limit.json" 2>"$TEST_ROOT/limit.err"; then
    fail "CORE-003B_limit was accepted"
fi
grep -Fq 'core.003b.limit' "$TEST_ROOT/limit.err" ||
    fail "CORE-003B_limit omitted its typed diagnostic"

cancel_status=0
SEEN_GLOBAL_INIT_TEST_HOOKS=1 python3 "$CHECKER" \
    "$FIXTURES/cancel/plan.json" --test-cancel-after-read \
    >"$TEST_ROOT/cancel.json" 2>"$TEST_ROOT/cancel.err" || cancel_status=$?
[ "$cancel_status" -eq 130 ] || fail "CORE-003B_cancel returned $cancel_status"
[ ! -s "$TEST_ROOT/cancel.json" ] || fail "CORE-003B_cancel emitted partial output"
grep -Fq 'core.003b.cancelled' "$TEST_ROOT/cancel.err" ||
    fail "CORE-003B_cancel omitted its typed diagnostic"

python3 "$CHECKER" "$FIXTURES/happy/plan.json" \
    --fuzz-seconds "${SEEN_CORE_003B_FUZZ_SECONDS:-1}" --seed 1101 \
    >"$TEST_ROOT/fuzz.json" 2>"$TEST_ROOT/fuzz.err" ||
    fail "seed-1101 global-initialization fuzz"
cmp -s "$TEST_ROOT/fuzz.json" "$FIXTURES/happy/expected.json" ||
    fail "fuzz changed canonical output"
grep -Fq 'seed=1101' "$TEST_ROOT/fuzz.err" || fail "fuzz omitted seed evidence"

for symbol in GlobalInitializationPlan planDeterministicGlobalInitialization \
    renderGlobalInitializationPlan initializationIndices; do
    grep -Fq "$symbol" "$SOURCE" || fail "native API omitted $symbol"
done
for code in core.003b.invalid core.003b.limit core.003b.cancelled \
    core.003b.platform; do
    grep -Fq "$code" "$SOURCE" || fail "native API omitted $code"
done
grep -Fq 'subsystem: AsciiString{ value: "compiler.initialization" }' \
    "$DIAGNOSTICS" || fail "diagnostic subsystem is not stable"
for field in code subsystem operation message causes nativeCode retry redaction; do
    grep -Fq "$field" "$DIAGNOSTICS" || fail "diagnostic omitted $field"
done

[ "$(grep -Fc 'globalInitializationOrder' "$ENTRY")" -ge 3 ] ||
    fail "compile path does not consume CORE-003B order"
[ "$(grep -Fc 'GlobalInitializationOrder' "$ENTRY")" -ge 6 ] ||
    fail "check and JIT do not consume CORE-003B order"
[ "$(grep -Fc 'globalInitializationRanksFromOrder(' "$ENTRY")" -ge 4 ] ||
    fail "production paths do not use one initialization-rank helper"
grep -Fq 'GLOBAL_CONSTRUCTOR_PRIORITY_MAX_MODULES: Int = 4096' "$MODULE_EMIT" ||
    fail "constructor priorities are not bounded"
grep -Fq 'GLOBAL_CONSTRUCTOR_PRIORITY_BASE + initializationIndex' "$MODULE_EMIT" ||
    fail "constructor priority is not derived from module order"
if grep -Fq 'i32 65535, ptr @' "$MODULE_EMIT"; then
    fail "equal-priority constructor fallback remains"
fi
grep -Fq 'emitGlobalConstructorsWithFeatureStateImpl(' \
    "$TAIL_DRIVER" || fail "module index does not reach constructor emission"
grep -Fq 'emitGlobalConstructorsImpl(' "$FEATURE_STATE" ||
    fail "feature-state owner does not emit constructors"
if grep -En 'global_ctors.*(sort|repair|rewrite)|65535, ptr @' \
    "$ROOT_DIR/scripts/fix_ir.py" \
    >"$TEST_ROOT/repair.txt"; then
    fail "conflicting production global-constructor repair remains"
fi

for case_name in CORE-003B_happy CORE-003B_invalid CORE-003B_limit \
    CORE-003B_cancel CORE-003B_cleanup; do
    grep -Fq "$case_name" "$NATIVE_TEST" ||
        fail "native executable regression omitted $case_name"
done
grep -Fq 'planDeterministicGlobalInitialization' "$EXAMPLE" ||
    fail "API example does not call the public Result entry point"
for fixture in compiler_seen/tests/release/global_initialization.seen \
    compiler_seen/examples/global_initialization_plan.seen \
    tests/fixtures/core-003b/happy/runtime/app.seen; do
    grep -Fq "$fixture" "$STAGE1" || fail "Stage-1 acceptance omits $fixture"
done

python3 "$BENCHMARK" "$FIXTURES/happy/plan.json" \
    "$FIXTURES/happy/benchmark.json" >"$TEST_ROOT/benchmark.log" ||
    fail "5-warmup/30-sample benchmark"
grep -Fq 'warmups=5 samples=30 status=pass' "$TEST_ROOT/benchmark.log" ||
    fail "benchmark omitted policy evidence"

python3 -c 'import json,sys; value=json.load(open(sys.argv[1], encoding="utf-8")); assert value["components"]["compiler"]["object_cache_abi"] == "seen-object-cache-abi-v3"' \
    "$COMPATIBILITY" || fail "compatibility manifest does not bind initialization"
python3 -c 'import json,sys; value=json.load(open(sys.argv[1], encoding="utf-8")); assert value["components"]["compiler"]["object_cache_abi"] == "seen-object-cache-abi-v2"' \
    "$FROZEN_COMPATIBILITY" ||
    fail "frozen compiler compatibility is not immutable v2"
(cd "$ROOT_DIR" && sha256sum -c \
    "${FROZEN_COMPATIBILITY_HASH#$ROOT_DIR/}") >/dev/null ||
    fail "frozen compiler compatibility hash"
grep -Fq 'cp -pL "$frozen_compatibility"' \
    "$ROOT_DIR/scripts/safe_rebuild.sh" ||
    fail "bootstrap overlay does not copy frozen compatibility"
grep -Fq 'seen-global-initialization-plan-v1 dependency-first constructor schedule' \
    "$COMPATIBILITY_SCHEMA" || fail "compatibility schema omits initialization binding"
grep -Fq '### Deterministic global initialization' "$ARCHITECTURE" ||
    fail "compiler architecture omits CORE-003B"
grep -Fq 'native deterministic global-initialization planning' "$CHANGELOG" ||
    fail "changelog omits CORE-003B"

python3 "$ROOT_DIR/scripts/check_native_boundaries.py" "$LEDGER" >/dev/null ||
    fail "native-boundary ledger"
if grep -Eq '^[[:space:]]*extern[[:space:]]+fun' "$SOURCE" "$DIAGNOSTICS"; then
    fail "native initialization implementation introduced unreviewed FFI"
fi
python3 -c 'import json,sys; expected=json.load(open(sys.argv[1], encoding="utf-8")); assert expected == {"children": 0, "descriptors": 0, "tasks": 0, "temporary_files": []}' \
    "$FIXTURES/cleanup/expected.json" || fail "cleanup expectation is invalid"
[ -z "$(jobs -pr)" ] || fail "CORE-003B_cleanup leaked a child"
if find "$FIXTURES" -type f \( -name '*.tmp' -o -name '.*.tmp' \) \
    -print -quit | grep -q .; then
    fail "CORE-003B_cleanup found a temporary artifact"
fi

echo "PASS: deterministic global initialization contract"
