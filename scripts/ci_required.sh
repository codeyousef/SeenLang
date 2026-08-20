#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
cd -- "$ROOT_DIR"

containment_fail() {
    echo "ci-containment: core.001b.invalid: $*" >&2
    exit 126
}

[ "${SEEN_CI_CONTAINMENT_IN_SCOPE:-0}" = "1" ] ||
    containment_fail "required gates were started outside the contained CI entrypoint"
[ "${SEEN_LOW_MEMORY:-0}" = "1" ] ||
    containment_fail "low-memory mode is not active"
[ "${SEEN_JOBS:-0}" = "1" ] && [ "${SEEN_OPT_JOBS:-0}" = "1" ] ||
    containment_fail "compiler workers are not serial"

if ! scripts/run_in_hard_memory_scope.sh --verify-only >/dev/null; then
    containment_fail "live kernel scope read-back failed"
fi

python3 -m py_compile \
    scripts/check_native_boundaries.py \
    scripts/check_native_inventory.py \
    scripts/check_ci_workflows.py \
    scripts/check_ci_containment.py
python3 scripts/check_ci_workflows.py >/dev/null
python3 scripts/check_ci_containment.py \
    docs/architecture/ci-containment.json >/dev/null
tests/misc_root_tests/seen_native_boundaries_ledger.sh
tests/misc_root_tests/seen_native_inventory_gate.sh
tests/misc_root_tests/seen_ci_workflow_contract.sh
tests/misc_root_tests/seen_ci_containment_contract.sh
tests/misc_root_tests/seen_ci_required_wiring.sh
tests/misc_root_tests/seen_memory_guard_fail_closed.sh
tests/misc_root_tests/seen_low_task_helper_serialization.sh
git diff --check

echo "PASS: required CI gates"
