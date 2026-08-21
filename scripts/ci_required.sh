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
    scripts/benchmark_compatibility_manifest.py \
    scripts/benchmark_compatibility_runtime.py \
    scripts/benchmark_package_layout.py \
    scripts/benchmark_import_graph.py \
    scripts/check_compatibility_manifest.py \
    scripts/check_package_layout.py \
    scripts/check_import_graph.py \
    tests/misc_root_tests/seen_compatibility_manifest_unit.py \
    tests/misc_root_tests/seen_package_layout_unit.py \
    tests/misc_root_tests/seen_import_graph_unit.py \
    scripts/check_native_boundaries.py \
    scripts/check_native_inventory.py \
    scripts/check_ci_workflows.py \
    scripts/check_ci_containment.py
python3 scripts/check_ci_workflows.py >/dev/null
python3 scripts/check_ci_containment.py \
    docs/architecture/ci-containment.json >/dev/null
python3 scripts/check_compatibility_manifest.py \
    releases/compatibility-manifest.json >/dev/null
tests/misc_root_tests/seen_compatibility_manifest_contract.sh
tests/misc_root_tests/seen_compatibility_manifest_runtime.sh
tests/misc_root_tests/seen_package_layout_contract.sh
tests/misc_root_tests/seen_import_graph_contract.sh
python3 scripts/benchmark_compatibility_runtime.py \
    releases/compatibility-manifest.json \
    tests/fixtures/core-002b/happy/benchmark.json
python3 scripts/benchmark_package_layout.py \
    tests/fixtures/pkg-layout-001/happy/layout.json \
    tests/fixtures/pkg-layout-001/happy/benchmark.json
python3 scripts/benchmark_import_graph.py \
    tests/fixtures/core-003a/happy/graph.json \
    tests/fixtures/core-003a/happy/benchmark.json
tests/misc_root_tests/seen_native_boundaries_ledger.sh
tests/misc_root_tests/seen_native_inventory_gate.sh
tests/misc_root_tests/seen_ci_workflow_contract.sh
tests/misc_root_tests/seen_ci_containment_contract.sh
tests/misc_root_tests/seen_ci_required_wiring.sh
tests/misc_root_tests/seen_memory_guard_fail_closed.sh
tests/misc_root_tests/seen_low_task_helper_serialization.sh
git diff --check

echo "PASS: required CI gates"
