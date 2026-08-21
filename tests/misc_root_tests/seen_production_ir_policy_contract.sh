#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
FIXTURES="$ROOT_DIR/tests/fixtures/core-003c"
CHECKER="$ROOT_DIR/scripts/check_production_ir_policy.py"
BENCHMARK="$ROOT_DIR/scripts/benchmark_production_ir_policy.py"
SOURCE="$ROOT_DIR/compiler_seen/src/imports/graph.seen"
DIAGNOSTICS="$ROOT_DIR/compiler_seen/src/release/diagnostic_schema.seen"
ENTRY="$ROOT_DIR/compiler_seen/src/main_compiler.seen"
SAFE_REBUILD="$ROOT_DIR/scripts/safe_rebuild.sh"
RECOVERY="$ROOT_DIR/scripts/recovery_opt.sh"
WINDOWS_BUILD="$ROOT_DIR/scripts/build_windows.sh"
PREBUILD="$ROOT_DIR/scripts/seen_prebuild_gates.sh"
NATIVE_TEST="$ROOT_DIR/compiler_seen/tests/release/production_ir_policy.seen"
EXAMPLE="$ROOT_DIR/compiler_seen/examples/production_ir_policy.seen"
STAGE1="$ROOT_DIR/scripts/seen_stage1_acceptance.sh"
ARCHITECTURE="$ROOT_DIR/docs/compiler-architecture.md"
BOOTSTRAP_DOC="$ROOT_DIR/docs/bootstrap.md"
LIMITATIONS="$ROOT_DIR/docs/known-limitations.md"
CHANGELOG="$ROOT_DIR/CHANGELOG.md"
COMPATIBILITY_SCHEMA="$ROOT_DIR/schemas/compatibility-manifest.schema.json"
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

for symbol in ProductionIrPlan productionIrSha256IsCanonical \
    validateUnmodifiedProductionIr renderProductionIrPlan; do
    grep -Fq "$symbol" "$SOURCE" || fail "native API omitted $symbol"
done
for code in core.003c.invalid core.003c.limit core.003c.cancelled \
    core.003c.platform; do
    grep -Fq "$code" "$SOURCE" || fail "native API omitted $code"
done
grep -Fq 'subsystem: AsciiString{ value: "compiler.ir" }' "$DIAGNOSTICS" ||
    fail "diagnostic subsystem is not stable"
for field in code subsystem operation message causes nativeCode retry redaction; do
    grep -Fq "$field" "$DIAGNOSTICS" || fail "diagnostic omitted $field"
done

# Production compilers must reach the real optimizer before any mutation code.
grep -Fq 'unset SEEN_FROZEN_IR_COMPAT' "$SAFE_REBUILD" ||
    fail "caller-controlled frozen compatibility marker is not cleared"
gate_count=$(grep -Fc 'if [ "\${SEEN_FROZEN_IR_COMPAT:-0}" != "1" ]; then' \
    "$SAFE_REBUILD")
gate_line=$(grep -Fn 'if [ "\${SEEN_FROZEN_IR_COMPAT:-0}" != "1" ]; then' \
    "$SAFE_REBUILD" | tail -1 | cut -d: -f1)
fix_line=$(grep -Fn 'python3 "$SCRIPT_DIR/fix_ir.py" "\$arg"' \
    "$SAFE_REBUILD" | head -1 | cut -d: -f1)
[ "$gate_count" -ge 2 ] ||
    fail "Linux and macOS production optimizer passthroughs are not both gated"
[ -n "$gate_line" ] && [ -n "$fix_line" ] && [ "$gate_line" -lt "$fix_line" ] ||
    fail "frozen compatibility is not gated before mutation"
grep -Fq '"SEEN_FROZEN_IR_COMPAT=1"' "$SAFE_REBUILD" ||
    fail "exact frozen Linux compile does not opt in"
grep -Fq 'SEEN_FROZEN_IR_COMPAT=1 \' "$SAFE_REBUILD" ||
    fail "frozen compatibility invocation is missing"
if grep -Fq 'SEEN_SKIP_IR_FIXUPS' "$SAFE_REBUILD"; then
    fail "caller-selectable production repair bypass remains"
fi
if grep -Fq 'VERIFIED="$STAGE2"' "$SAFE_REBUILD"; then
    fail "repaired frozen Stage2 remains production-eligible"
fi
grep -Fq 'IR mutation is permitted only for captured frozen Stage-1 output' \
    "$RECOVERY" || fail "recovery does not reject unauthorized mutation"
if grep -Eq 'fix_ir|byteAt fix|\.dedup' "$WINDOWS_BUILD"; then
    fail "Windows production build still rewrites IR"
fi
if sed -n '/sweep_saved_ll_dir()/,/^}/p' "$PREBUILD" | grep -Fq 'fix_ir.py'; then
    fail "saved production IR preflight still repairs before verification"
fi
if grep -Eq 'fix_ir\.py|rewrite_codegen_tmp\.py' "$ENTRY"; then
    fail "compiler entrypoint invokes a production repair script"
fi
for diagnostic in \
    'core.003c.invalid: optimizer rejected unmodified IR' \
    'core.003c.platform: object emission failed for unmodified IR'; do
    grep -Fq "$diagnostic" "$ENTRY" || fail "compiler omitted $diagnostic"
done

for case_name in CORE-003C_happy CORE-003C_invalid CORE-003C_limit \
    CORE-003C_cancel CORE-003C_cleanup; do
    grep -Fq "$case_name" "$NATIVE_TEST" ||
        fail "native executable regression omitted $case_name"
done
grep -Fq 'validateUnmodifiedProductionIr' "$EXAMPLE" ||
    fail "API example does not call the public Result entry point"
for fixture in compiler_seen/tests/release/production_ir_policy.seen \
    compiler_seen/examples/production_ir_policy.seen; do
    grep -Fq "$fixture" "$STAGE1" || fail "Stage-1 acceptance omits $fixture"
done

python3 "$BENCHMARK" "$FIXTURES/happy/policy.json" \
    "$FIXTURES/happy/benchmark.json" >"$TEST_ROOT/benchmark.log" ||
    fail "5-warmup/30-sample benchmark"
grep -Fq 'warmups=5 samples=30 status=pass' "$TEST_ROOT/benchmark.log" ||
    fail "benchmark omitted policy evidence"

grep -Fq 'seen-production-ir-policy-v1' "$COMPATIBILITY_SCHEMA" ||
    fail "compatibility schema omits the production-IR policy binding"
grep -Fq '### Unmodified production IR' "$ARCHITECTURE" ||
    fail "compiler architecture omits CORE-003C"
grep -Fq 'frozen Stage-1 IR compatibility' "$BOOTSTRAP_DOC" ||
    fail "bootstrap documentation does not isolate frozen compatibility"
grep -Fq 'never production-eligible' "$LIMITATIONS" ||
    fail "known limitations do not exclude repaired IR from production"
grep -Fq 'unmodified production IR' "$CHANGELOG" ||
    fail "changelog omits CORE-003C"

python3 "$ROOT_DIR/scripts/check_native_boundaries.py" "$LEDGER" >/dev/null ||
    fail "native-boundary ledger"
if grep -Eq '^[[:space:]]*extern[[:space:]]+fun' "$SOURCE" "$DIAGNOSTICS"; then
    fail "native production-IR policy introduced unreviewed FFI"
fi
python3 -c 'import json,sys; expected=json.load(open(sys.argv[1], encoding="utf-8")); assert expected == {"children": 0, "descriptors": 0, "tasks": 0, "temporary_files": []}' \
    "$FIXTURES/cleanup/expected.json" || fail "cleanup expectation is invalid"
[ -z "$(jobs -pr)" ] || fail "CORE-003C_cleanup leaked a child"
if find "$FIXTURES" -type f \( -name '*.tmp' -o -name '.*.tmp' \) \
    -print -quit | grep -q .; then
    fail "CORE-003C_cleanup found a temporary artifact"
fi

echo "PASS: unmodified production IR policy contract"
