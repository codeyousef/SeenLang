#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
WORKFLOW="$ROOT_DIR/.github/workflows/ci.yml"
REQUIRED="$ROOT_DIR/scripts/ci_required.sh"

fail() {
    echo "FAIL: required CI wiring: $*" >&2
    exit 1
}

[ -f "$WORKFLOW" ] && [ ! -L "$WORKFLOW" ] || fail "active workflow is missing or unsafe"
[ -x "$REQUIRED" ] && [ ! -L "$REQUIRED" ] || fail "required gate is missing or not executable"
bash -n "$REQUIRED" || fail "required gate shell syntax"

grep -Fxq 'name: CI' "$WORKFLOW" || fail "workflow name is not CI"
grep -Fxq '  required:' "$WORKFLOW" || fail "required job id is missing"
grep -Fxq '    name: required' "$WORKFLOW" || fail "required check name is missing"
grep -Eq '^[[:space:]]+uses: actions/checkout@[0-9a-f]{40}$' "$WORKFLOW" ||
    fail "checkout action is not commit-pinned"
grep -Fxq '        run: scripts/ci_required.sh' "$WORKFLOW" ||
    fail "workflow does not invoke the required gate"

for required_command in \
    'python3 scripts/check_ci_workflows.py' \
    'tests/misc_root_tests/seen_native_boundaries_ledger.sh' \
    'tests/misc_root_tests/seen_native_inventory_gate.sh' \
    'tests/misc_root_tests/seen_ci_workflow_contract.sh' \
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

if rg -n 'safe_rebuild|compiler_seen/target/seen|(^|[[:space:]])sudo([[:space:]]|$)|/tmp/' \
    "$WORKFLOW" "$REQUIRED"; then

    fail "static required CI unexpectedly invokes a build, privilege escalation, or host temporary path"
fi

echo "PASS: required CI wiring"
