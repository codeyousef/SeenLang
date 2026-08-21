#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
WORKFLOW="$ROOT_DIR/.github/workflows/ci.yml"
REQUIRED="$ROOT_DIR/scripts/ci_required.sh"
CONTAINED="$ROOT_DIR/scripts/run_ci_required.sh"

fail() {
    echo "FAIL: required CI wiring: $*" >&2
    exit 1
}

[ -f "$WORKFLOW" ] && [ ! -L "$WORKFLOW" ] || fail "active workflow is missing or unsafe"
[ -x "$REQUIRED" ] && [ ! -L "$REQUIRED" ] || fail "required gate is missing or not executable"
[ -x "$CONTAINED" ] && [ ! -L "$CONTAINED" ] || fail "contained CI entrypoint is missing or not executable"
bash -n "$REQUIRED" || fail "required gate shell syntax"
bash -n "$CONTAINED" || fail "contained CI entrypoint shell syntax"

set +e
outside_error=$(env -u SEEN_CI_CONTAINMENT_IN_SCOPE "$REQUIRED" 2>&1 >/dev/null)
outside_status=$?
set -e
[ "$outside_status" -eq 126 ] ||
    fail "inner required gate did not fail closed outside containment"
case "$outside_error" in
    *core.001b.invalid*) ;;
    *) fail "outside-containment rejection omitted its typed diagnostic" ;;
esac

grep -Fxq 'name: CI' "$WORKFLOW" || fail "workflow name is not CI"
grep -Fxq '  required:' "$WORKFLOW" || fail "required job id is missing"
grep -Fxq '    name: required' "$WORKFLOW" || fail "required check name is missing"
grep -Eq '^[[:space:]]+uses: actions/checkout@[0-9a-f]{40}$' "$WORKFLOW" ||
    fail "checkout action is not commit-pinned"
grep -Fxq '        run: scripts/run_ci_required.sh' "$WORKFLOW" ||
    fail "workflow does not invoke the required gate"

for required_command in \
    'python3 scripts/check_compatibility_manifest.py' \
    'tests/misc_root_tests/seen_import_graph_contract.sh' \
    'tests/misc_root_tests/seen_global_initialization_contract.sh' \
    'tests/misc_root_tests/seen_production_ir_policy_contract.sh' \
    'tests/misc_root_tests/seen_production_source_policy_contract.sh' \
    'tests/misc_root_tests/seen_machine_diagnostic_contract.sh' \
    'tests/misc_root_tests/seen_build_instrumentation_contract.sh' \
    'tests/misc_root_tests/seen_release_optimization_contract.sh' \
    'python3 scripts/check_ci_workflows.py' \
    'python3 scripts/check_ci_containment.py' \
    'tests/misc_root_tests/seen_compatibility_manifest_contract.sh' \
    'tests/misc_root_tests/seen_native_boundaries_ledger.sh' \
    'tests/misc_root_tests/seen_native_inventory_gate.sh' \
    'tests/misc_root_tests/seen_ci_workflow_contract.sh' \
    'tests/misc_root_tests/seen_ci_containment_contract.sh' \
    'git diff --check'; do

    grep -Fq "$required_command" "$REQUIRED" ||
        fail "required gate omitted $required_command"
done

[ ! -e "$ROOT_DIR/.github/workflows-disabled" ] ||
    fail "retired workflow directory remains"
if find "$ROOT_DIR/.github/workflows" -maxdepth 1 -type f -name '*.disabled' -print -quit |
    grep -q .; then
    fail "retired disabled workflow remains active in the repository"
fi

grep -Fq 'scripts/run_in_hard_memory_scope.sh' "$CONTAINED" ||
    fail "contained CI entrypoint does not enter the hard scope"
grep -Fq 'SEEN_CI_CONTAINMENT_IN_SCOPE=1' "$CONTAINED" ||
    fail "contained CI entrypoint does not identify the inner gate"
grep -Fq 'scripts/run_in_hard_memory_scope.sh --verify-only' "$REQUIRED" ||
    fail "inner required gate does not re-verify the live hard scope"

if grep -En 'safe_rebuild|compiler_seen/target/seen|(^|[[:space:]])sudo([[:space:]]|$)|/tmp/' \
    "$WORKFLOW" "$REQUIRED" "$CONTAINED"; then

    fail "static required CI unexpectedly invokes a build, privilege escalation, or host temporary path"
fi

echo "PASS: required CI wiring"
