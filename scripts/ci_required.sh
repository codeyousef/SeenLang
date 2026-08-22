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
    scripts/benchmark_global_initialization.py \
    scripts/benchmark_production_ir_policy.py \
    scripts/benchmark_production_source_policy.py \
    scripts/benchmark_machine_diagnostic.py \
    scripts/benchmark_build_instrumentation.py \
    scripts/benchmark_release_optimization.py \
    scripts/benchmark_test_discovery.py \
    scripts/check_test_runner.py \
    scripts/check_test_snapshots.py \
    scripts/check_test_fixture.py \
    scripts/check_test_reporters.py \
    scripts/check_test_migration.py \
    scripts/check_compatibility_manifest.py \
    scripts/check_package_layout.py \
    scripts/check_import_graph.py \
    scripts/check_global_initialization.py \
    scripts/check_production_ir_policy.py \
    scripts/check_production_source_policy.py \
    scripts/check_machine_diagnostic.py \
    scripts/check_build_instrumentation.py \
    scripts/check_release_optimization.py \
    scripts/discover_seen_tests.py \
    tests/misc_root_tests/seen_compatibility_manifest_unit.py \
    tests/misc_root_tests/seen_package_layout_unit.py \
    tests/misc_root_tests/seen_import_graph_unit.py \
    tests/misc_root_tests/seen_global_initialization_unit.py \
    tests/misc_root_tests/seen_production_ir_policy_unit.py \
    tests/misc_root_tests/seen_production_source_policy_unit.py \
    tests/misc_root_tests/seen_machine_diagnostic_unit.py \
    tests/misc_root_tests/seen_build_instrumentation_unit.py \
    tests/misc_root_tests/seen_release_optimization_unit.py \
    tests/runner/test_discovery_unit.py \
    tests/runner/test_runner_unit.py \
    tests/runner/test_snapshots_unit.py \
    tests/runner/test_fixture_unit.py \
    tests/runner/test_reporters_unit.py \
    tests/runner/test_migration_unit.py \
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
tests/misc_root_tests/seen_global_initialization_contract.sh
tests/misc_root_tests/seen_production_ir_policy_contract.sh
tests/misc_root_tests/seen_production_source_policy_contract.sh
tests/misc_root_tests/seen_machine_diagnostic_contract.sh
tests/misc_root_tests/seen_build_instrumentation_contract.sh
tests/misc_root_tests/seen_release_optimization_contract.sh
tests/misc_root_tests/seen_test_discovery_contract.sh
tests/misc_root_tests/seen_test_runner_contract.sh
tests/misc_root_tests/seen_assertions_snapshot_contract.sh
tests/misc_root_tests/seen_fixture_isolation_contract.sh
tests/misc_root_tests/seen_test_reporters_contract.sh
tests/misc_root_tests/seen_test_migration_contract.sh
python3 scripts/benchmark_compatibility_runtime.py \
    releases/compatibility-manifest.json \
    tests/fixtures/core-002b/happy/benchmark.json
python3 scripts/benchmark_package_layout.py \
    tests/fixtures/pkg-layout-001/happy/layout.json \
    tests/fixtures/pkg-layout-001/happy/benchmark.json
tests/misc_root_tests/seen_native_boundaries_ledger.sh
tests/misc_root_tests/seen_native_inventory_gate.sh
tests/misc_root_tests/seen_ci_workflow_contract.sh
tests/misc_root_tests/seen_ci_containment_contract.sh
tests/misc_root_tests/seen_ci_required_wiring.sh
tests/misc_root_tests/seen_memory_guard_fail_closed.sh
tests/misc_root_tests/seen_low_task_helper_serialization.sh
git diff --check

echo "PASS: required CI gates"
