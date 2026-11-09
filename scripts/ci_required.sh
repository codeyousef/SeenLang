#!/usr/bin/env bash

set -euo pipefail

# Match the conventional GitHub-hosted Linux runner stack. Inheriting a
# larger developer-shell limit can hide release-mode stack regressions.
if ! ulimit -S -s 8192 2>/dev/null; then
    echo "ci-containment: core.001b.invalid: could not set the required 8192 KiB stack limit" >&2
    exit 126
fi
if [ "$(ulimit -S -s)" != "8192" ]; then
    echo "ci-containment: core.001b.invalid: required CI stack limit read-back failed" >&2
    exit 126
fi

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

case "${SEEN_CI_PHASE:-all}" in
    all|bootstrap) ;;
    acceptance)
        exec "$ROOT_DIR/scripts/run_ci_acceptance_group.sh" \
            "${SEEN_CI_ACCEPTANCE_GROUP:-}"
        ;;
    native-arm-inputs)
        exec bash "$ROOT_DIR/scripts/build_linux_arm64_verification_inputs.sh" \
            --candidate "$ROOT_DIR/target/seen-build/bootstrap-candidate" \
            --output "$ROOT_DIR/target/seen-build/linux-arm64-inputs.tar.gz"
        ;;
    native-windows-inputs)
        exec bash "$ROOT_DIR/scripts/build_windows_iocp_inputs.sh" \
            --candidate "$ROOT_DIR/target/seen-build/bootstrap-candidate" \
            --output "$ROOT_DIR/target/seen-build/windows-iocp-inputs.tar.gz"
        ;;
    native-arm)
        mkdir -- "$ROOT_DIR/target/seen-build/linux-arm64-qualification"
        exec bash "$ROOT_DIR/scripts/verify_linux_arm64_milestone.sh" \
            --candidate "$ROOT_DIR/target/seen-build/bootstrap-candidate" \
            --archive "$ROOT_DIR/target/seen-build/linux-arm64-transfer/linux-arm64-inputs.tar.gz" \
            --report "$ROOT_DIR/target/seen-build/linux-arm64-qualification/report.json"
        ;;
    finalize)
        python3 "$ROOT_DIR/scripts/linux_arm64_milestone_report.py" verify-report \
            --root "$ROOT_DIR" \
            --candidate "$ROOT_DIR/target/seen-build/bootstrap-candidate" \
            --archive "$ROOT_DIR/target/seen-build/linux-arm64-transfer/linux-arm64-inputs.tar.gz" \
            --report "$ROOT_DIR/target/seen-build/linux-arm64-qualification/report.json"
        python3 "$ROOT_DIR/scripts/ci_windows_iocp.py" verify-report \
            --root "$ROOT_DIR" \
            --candidate "$ROOT_DIR/target/seen-build/bootstrap-candidate" \
            --archive "$ROOT_DIR/target/seen-build/windows-iocp-transfer/windows-iocp-inputs.tar.gz" \
            --report "$ROOT_DIR/target/seen-build/windows-iocp-qualification/report.json"
        "$ROOT_DIR/scripts/certify_gate0_clean_checkout.sh" --finalize-candidate
        # Release preparation includes real package/compiler verification.
        # Keep that owned work inside the same bounded scope as finalization.
        exec "$ROOT_DIR/scripts/prepare_release_toolchain_artifact.sh"
        ;;
    *) containment_fail "unknown required CI phase" ;;
esac

# CORE-004E/F/G: reject malformed deterministic contracts before any bootstrap
# or clean-checkout certification work. CORE-004E execution is deliberately
# deferred to Stage-1 acceptance, where the verified fresh compiler is explicit.
bash -n \
    scripts/run_ci_acceptance_group.sh \
    scripts/provision_ci_host.sh \
    scripts/provision_ci_linux_arm64_host.sh \
    scripts/ci_linux_namespace_preflight.sh \
    scripts/build_linux_arm64_verification_inputs.sh \
    scripts/verify_linux_arm64_milestone.sh \
    scripts/build_windows_iocp_inputs.sh \
    scripts/verify_ci_native_host_inputs.sh \
    scripts/memory_budget.sh \
    scripts/seen_stage1_acceptance.sh \
    scripts/certify_gate0_clean_checkout.sh \
    scripts/run_attested_seen.sh \
    scripts/installed_payload_fixture_worker.sh \
    tests/misc_root_tests/seen_core_004e_deterministic_context.sh \
    tests/misc_root_tests/seen_program_artifacts_contract.sh \
    tests/misc_root_tests/seen_program_reproducibility_contract.sh \
    tests/misc_root_tests/seen_v019_float_codegen_contract.sh \
    tests/misc_root_tests/seen_result_aggregate_array_data_contract.sh \
    tests/misc_root_tests/seen_result_scalar_payload_contract.sh \
    tests/misc_root_tests/seen_inf_simd_a.sh \
    tests/misc_root_tests/seen_async_io_a.sh \
    tests/misc_root_tests/seen_async_io_b.sh \
    tests/misc_root_tests/seen_async_io_c.sh \
    tests/misc_root_tests/seen_async_io_d.sh \
    tests/misc_root_tests/seen_async_io_e.sh \
    tests/misc_root_tests/seen_async_io_f.sh \
    tests/misc_root_tests/seen_inf_simd_b.sh \
    tests/misc_root_tests/seen_inf_simd_c.sh \
    tests/misc_root_tests/seen_inf_simd_d.sh \
    tests/misc_root_tests/seen_inf_simd_e.sh \
    tests/misc_root_tests/seen_inf_simd_f.sh \
    tests/misc_root_tests/seen_p0_accel_gate_a.sh \
    tests/misc_root_tests/seen_p0_accel_gate_b.sh \
    tests/misc_root_tests/seen_p0_accel_gate_c.sh \
    tests/misc_root_tests/seen_p0_accel_gate_d.sh \
    tests/expanded_gate_0_accelerator_certification/cpu_isolation/tool_trap.sh \
    tests/misc_root_tests/seen_gpu_ir_a.sh \
    tests/misc_root_tests/seen_gpu_ir_b.sh \
    tests/misc_root_tests/seen_gpu_ir_c.sh \
    tests/misc_root_tests/seen_gpu_ir_d.sh \
    tests/misc_root_tests/seen_gpu_ir_d_input_ownership.sh \
    tests/misc_root_tests/seen_gpu_ir_e.sh \
    tests/misc_root_tests/seen_gpu_spirv_a.sh \
    tests/misc_root_tests/seen_gpu_spirv_b.sh \
    tests/misc_root_tests/seen_gpu_spirv_c.sh \
    tests/misc_root_tests/seen_gpu_ptx_a.sh \
    tests/misc_root_tests/seen_gpu_ptx_b.sh \
    tests/misc_root_tests/seen_gpu_ptx_b_numeric.sh \
    tests/misc_root_tests/seen_gpu_ptx_c.sh \
    tests/misc_root_tests/seen_gpu_amdgpu_a.sh \
    tests/misc_root_tests/seen_gpu_amdgpu_b.sh \
    tests/misc_root_tests/seen_gpu_amdgpu_b_numeric.sh \
    tests/misc_root_tests/seen_gpu_amdgpu_c.sh \
    tests/misc_root_tests/seen_gpu_metal_a.sh \
    tests/misc_root_tests/seen_gpu_metal_b.sh \
    tests/misc_root_tests/seen_gpu_metal_b_numeric.sh \
    tests/misc_root_tests/seen_gpu_metal_c.sh \
    tests/misc_root_tests/seen_gpu_runtime_a.sh \
    tests/misc_root_tests/seen_gpu_runtime_a_vulkan.sh \
    tests/misc_root_tests/seen_gpu_runtime_b.sh \
    tests/misc_root_tests/seen_gpu_runtime_c.sh \
    tests/misc_root_tests/seen_gpu_vulkan_mapping.sh \
    tests/misc_root_tests/seen_gpu_vulkan_transfer.sh \
    tests/misc_root_tests/seen_gpu_vulkan_staging.sh \
    tests/misc_root_tests/seen_gpu_runtime_d.sh \
    tests/misc_root_tests/seen_gpu_runtime_e.sh \
    tests/misc_root_tests/seen_gpu_runtime_f.sh \
    tests/misc_root_tests/seen_gpu_runtime_g.sh \
    tests/misc_root_tests/seen_gpu_runtime_h.sh \
    tests/misc_root_tests/seen_cuda_event_query.sh \
    tests/misc_root_tests/seen_gpu_cuda_staging.sh \
    tests/misc_root_tests/seen_gpu_cuda_peer.sh \
    tests/misc_root_tests/seen_fs_close_error.sh \
    tests/misc_root_tests/seen_scalar_print_ownership.sh \
    tests/misc_root_tests/seen_fs_file_ownership.sh \
    tests/misc_root_tests/seen_json_float_serialization.sh \
    tests/misc_root_tests/seen_generic_reprc_abi.sh \
    tests/misc_root_tests/seen_prebuilt_generic_reprc_abi.sh \
    tests/misc_root_tests/seen_compiled_fixture_status.sh \
    tests/misc_root_tests/seen_compiled_fixture_metadata_linux_x86_64.sh \
    tests/misc_root_tests/seen_ir_only_phase_linux_x86_64.sh \
    tests/misc_root_tests/seen_select_guard_safety.sh \
    scripts/vulkan_reference_fixture_support.sh \
    tests/misc_root_tests/seen_dyn_trait_void_contract.sh \
    scripts/compiled_fixture_support.sh \
    tests/misc_root_tests/seen_integer_string_width_contract.sh \
    tests/misc_root_tests/seen_aggregate_return_cuda_stdlib_contract.sh \
    tests/misc_root_tests/seen_pointer_field_cast_contract.sh \
    tests/misc_root_tests/seen_repr_c_aggregate_argument_contract.sh \
    tests/misc_root_tests/seen_result_repr_c_literal_contract.sh \
    tests/misc_root_tests/seen_repr_c_aggregate_field_assignment_contract.sh \
    tests/misc_root_tests/seen_nested_repr_c_aggregate_address_contract.sh \
    tests/misc_root_tests/seen_transitive_repr_c_aggregate_layout_contract.sh \
    tests/misc_root_tests/seen_repr_c_aggregate_array_push_contract.sh \
    tests/misc_root_tests/seen_class_destruction_accounting_contract.sh \
    tests/misc_root_tests/seen_option_free_layout_contract.sh \
    tests/misc_root_tests/seen_option_payload_width_contract.sh \
    tests/misc_root_tests/seen_result_string_unwrap_contract.sh \
    tests/misc_root_tests/seen_result_payload_pool_live_contract.sh \
    tests/misc_root_tests/seen_runtime_cstring_accounting.sh \
    tests/misc_root_tests/seen_unsigned_right_shift_contract.sh \
    tests/misc_root_tests/seen_negative_default_arguments_contract.sh \
    tests/misc_root_tests/seen_bundled_cuda_link.sh \
    tests/misc_root_tests/seen_cuda_after_bounded_array_release.sh \
    tests/misc_root_tests/seen_async_structured_contract.sh \
    tests/misc_root_tests/seen_multithread_executor_contract.sh \
    tests/misc_root_tests/seen_async_synchronization_contract.sh \
    tests/misc_root_tests/seen_cpu_blocking_pools_contract.sh \
    tests/misc_root_tests/seen_async_reactor_contract.sh \
    tests/misc_root_tests/seen_async_streams_process_dns_contract.sh \
    tests/misc_root_tests/seen_task_context_policy_contract.sh \
    tests/misc_root_tests/seen_async_stress_contract.sh \
    tests/misc_root_tests/seen_stdlib_component_payload.sh \
    tests/misc_root_tests/seen_mapped_close_ownership.sh \
    tests/misc_root_tests/seen_open_issue_regressions.sh \
    tests/misc_root_tests/seen_windows_abi_transform.sh \
    tests/misc_root_tests/seen_macos_package_client_packaging.sh \
    tests/misc_root_tests/test_release_packaging.sh
tests/misc_root_tests/seen_program_artifacts_contract.sh
SEEN_CORE_004G_FUZZ_SECONDS=0.01 \
    tests/misc_root_tests/seen_program_reproducibility_contract.sh

python3 -m py_compile \
    scripts/accelerator_gate_evidence.py \
    tests/misc_root_tests/seen_accelerator_gate_evidence_contract.py \
    tests/expanded_gate_0_accelerator_certification/cpu_isolation/probe_evidence.py \
    tests/expanded_gate_0_accelerator_certification/logical_reflection.py \
    tests/expanded_gate_0_accelerator_certification/runtime_child_evidence.py \
    tests/expanded_gate_0_accelerator_certification/hardware_evidence.py \
    tests/expanded_gate_0_accelerator_certification/metal_child_evidence.py \
    scripts/check_gpu_spirv_artifacts.py \
    scripts/check_gpu_spirv_toolchain.py \
    scripts/gpu_toolchain_process.py \
    scripts/check_gpu_ptx_toolchain.py \
    scripts/check_gpu_ptx_artifacts.py \
    scripts/install_gpu_ptx_toolchain.py \
    scripts/certify_gpu_spirv.py \
    scripts/certify_gpu_ptx.py \
    scripts/certify_gpu_amdgpu.py \
    scripts/check_gpu_amdgpu_toolchain.py \
    scripts/check_gpu_amdgpu_artifacts.py \
    scripts/check_gpu_amdgpu_scalar.py \
    scripts/check_gpu_metal_artifacts.py \
    scripts/check_gpu_metal_scalar.py \
    scripts/check_gpu_metal_toolchain.py \
    scripts/certify_gpu_metal.py \
    tests/gpu_artifacts/metal_certification_contract.py \
    tests/gpu_artifacts/metal_toolchain_lifecycle_contract.py \
    scripts/gpu_metal_host.py \
    scripts/gpu_scalar_corpus.py \
    tests/gpu_artifacts/metal_metadata_contract.py \
    scripts/check_gpu_amdgpu_capabilities.py \
    scripts/check_gpu_amdgpu_numerics.py \
    scripts/check_gpu_amdgpu_subgroups.py \
    scripts/check_gpu_amdgpu_cooperative.py \
    scripts/check_gpu_amdgpu_native.py \
    scripts/check_simd_packed_allocations.py \
    scripts/gpu_launch_contract.py \
    scripts/gpu_certification_io.py \
    scripts/gpu_certification_profiles.py \
    tests/gpu_artifacts/certification_io_contract.py \
    tests/gpu_artifacts/amdgpu_metadata_contract.py \
    scripts/cpu_benchmark_statistics.py \
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
    scripts/check_error_contract.py \
    scripts/benchmark_error_contract.py \
    scripts/check_typed_errors.py \
    scripts/benchmark_typed_errors.py \
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
    tests/runner/test_error_contract_unit.py \
    tests/runner/test_typed_errors_unit.py \
    tests/runner/test_benchmark_statistics_unit.py \
    scripts/check_native_boundaries.py \
    scripts/check_native_inventory.py \
    scripts/check_ci_workflows.py \
    scripts/finite_tsan_sdk.py \
    scripts/finite_tsan_sdk_source.py \
    scripts/finite_tsan_sdk_exec.py \
    scripts/core_004e_crash_witness.py \
    tests/runner/test_finite_tsan_sdk_unit.py \
    tests/runner/test_finite_tsan_sdk_exec_unit.py \
    tests/misc_root_tests/core_004e_crash_witness_contract.py \
    scripts/ci_bootstrap_candidate.py \
    scripts/ci_candidate_io.py \
    scripts/ci_linux_arm64_inputs.py \
    scripts/ci_windows_iocp.py \
    scripts/linux_arm64_milestone_report.py \
    scripts/linux_arm64_asan_admission.py \
    scripts/gpu_driver_leak_attribution.py \
    tests/runner/test_ci_linux_arm64_inputs_unit.py \
    tests/runner/test_ci_windows_iocp_unit.py \
    tests/runner/test_linux_arm64_milestone_unit.py \
    tests/runner/test_linux_arm64_asan_admission_unit.py \
    tests/runner/test_gpu_driver_leak_attribution_unit.py \
    tests/runner/test_ci_bootstrap_candidate_unit.py \
    tests/runner/test_memory_budget_unit.py \
    tests/runner/test_ci_workflow_policy_unit.py \
    scripts/check_release_ci_run.py \
    scripts/release_toolchain_artifact.py \
    scripts/release_platform_inputs.py \
    scripts/release_draft_api.py \
    scripts/check_ci_containment.py \
    scripts/check_gate0_certification.py \
    scripts/check_qwen_contracts.py \
    scripts/benchmark_gate0_certification.py \
    scripts/measure_gate0_certification_coverage.py \
    tests/runner/test_gate0_certification_unit.py \
    tests/runner/test_release_ci_run_unit.py \
    tests/runner/test_release_toolchain_artifact_unit.py \
    tests/runner/test_release_platform_inputs_unit.py \
    tests/runner/test_release_draft_api_unit.py
python3 tests/misc_root_tests/seen_accelerator_gate_evidence_contract.py
python3 tests/gpu_artifacts/metal_certification_contract.py
python3 tests/gpu_artifacts/metal_toolchain_lifecycle_contract.py
python3 -m unittest \
    tests.runner.test_ci_bootstrap_candidate_unit \
    tests.runner.test_ci_linux_arm64_inputs_unit \
    tests.runner.test_ci_windows_iocp_unit \
    tests.runner.test_linux_arm64_milestone_unit \
    tests.runner.test_linux_arm64_asan_admission_unit \
    tests.runner.test_gpu_driver_leak_attribution_unit \
    tests.runner.test_ci_workflow_policy_unit \
    tests.runner.test_memory_budget_unit \
    tests.runner.test_finite_tsan_sdk_unit \
    tests.runner.test_finite_tsan_sdk_exec_unit \
    tests.runner.test_release_ci_run_unit \
    tests.runner.test_release_toolchain_artifact_unit \
    tests.runner.test_release_platform_inputs_unit \
    tests.runner.test_release_draft_api_unit
python3 -m unittest tests.runner.test_repair_frozen_coro_ir_unit
python3 -m unittest tests.runner.test_ir_failure_diagnostics_unit
python3 -m py_compile \
    scripts/check_release_artifact_manifest.py \
    scripts/benchmark_release_artifact_manifest.py \
    scripts/measure_release_artifact_manifest_coverage.py \
    tests/runner/test_release_artifact_manifest_unit.py
python3 scripts/check_ci_workflows.py >/dev/null
python3 tests/misc_root_tests/verification_log_retention_contract.py
python3 tests/misc_root_tests/core_004e_crash_witness_contract.py
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
tests/misc_root_tests/seen_error_contract.sh
tests/misc_root_tests/seen_typed_errors_contract.sh
python3 -m unittest tests.runner.test_benchmark_statistics_unit
tests/misc_root_tests/seen_cpu_benchmark_clock_contract.sh
tests/misc_root_tests/seen_error_api_migration.sh
tests/misc_root_tests/seen_error_policy_contract.sh
tests/misc_root_tests/seen_owned_resource_contract.sh
tests/misc_root_tests/seen_mapped_close_ownership.sh
tests/misc_root_tests/seen_secret_marker_contract.sh
tests/misc_root_tests/seen_test_instrumentation_contract.sh
tests/misc_root_tests/seen_fuzz_corpus_contract.sh
tests/misc_root_tests/seen_benchmark_evidence_contract.sh
tests/misc_root_tests/seen_leak_soak_contract.sh
tests/misc_root_tests/seen_bootstrap_reproducibility_contract.sh
tests/misc_root_tests/seen_release_artifact_pins_contract.sh
tests/misc_root_tests/seen_stdlib_component_payload.sh
tests/misc_root_tests/seen_compiler_provenance_contract.sh
tests/misc_root_tests/seen_release_upload_artifact_scope.sh
tests/misc_root_tests/seen_windows_abi_transform.sh
tests/misc_root_tests/seen_macos_package_client_packaging.sh
tests/misc_root_tests/test_release_packaging.sh
tests/misc_root_tests/seen_gate0_certification_contract.sh
python3 scripts/benchmark_compatibility_runtime.py \
    releases/compatibility-manifest.json \
    tests/fixtures/core-002b/happy/benchmark.json
python3 scripts/benchmark_package_layout.py \
    tests/fixtures/pkg-layout-001/happy/layout.json \
    tests/fixtures/pkg-layout-001/happy/benchmark.json
tests/misc_root_tests/seen_native_boundaries_ledger.sh
tests/misc_root_tests/seen_native_inventory_gate.sh
tests/misc_root_tests/seen_sync_contract.sh
tests/misc_root_tests/seen_sync_native_abi.sh
tests/misc_root_tests/seen_executor_native_abi.sh
tests/misc_root_tests/seen_ci_workflow_contract.sh
tests/misc_root_tests/seen_ci_containment_contract.sh
tests/misc_root_tests/seen_ci_required_boundary.sh
tests/misc_root_tests/seen_executable_baseline_contract.sh
tests/misc_root_tests/seen_memory_guard_fail_closed.sh
tests/misc_root_tests/seen_low_task_helper_serialization.sh
git diff --check

if [ "${SEEN_CI_PHASE:-all}" = bootstrap ]; then
    scripts/certify_gate0_clean_checkout.sh --bootstrap-candidate
else
    scripts/certify_gate0_clean_checkout.sh
fi

echo "PASS: required CI phase ${SEEN_CI_PHASE:-all}"
