#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
WORKFLOW="$ROOT_DIR/.github/workflows/ci.yml"
RELEASE_WORKFLOW="$ROOT_DIR/.github/workflows/release.yml"
REQUIRED="$ROOT_DIR/scripts/ci_required.sh"
CONTAINED="$ROOT_DIR/scripts/run_ci_required.sh"
PROVISION="$ROOT_DIR/scripts/provision_ci_host.sh"
RELEASE_INNER="$ROOT_DIR/scripts/build_and_upload_release.sh"
RELEASE_CONTAINED="$ROOT_DIR/scripts/run_release_upload.sh"
RELEASE_CI="$ROOT_DIR/scripts/verify_release_ci_run.sh"
RELEASE_CI_CHECKER="$ROOT_DIR/scripts/check_release_ci_run.py"
RELEASE_TOOLCHAIN="$ROOT_DIR/scripts/release_toolchain_artifact.py"
PREPARE_RELEASE_TOOLCHAIN="$ROOT_DIR/scripts/prepare_release_toolchain_artifact.sh"
RELEASE_TAG_POLICY="$ROOT_DIR/scripts/release_tag_policy.sh"
PREBUILD="$ROOT_DIR/scripts/seen_prebuild_gates.sh"
STAGE1="$ROOT_DIR/scripts/seen_stage1_acceptance.sh"
CORE_004E="$ROOT_DIR/tests/misc_root_tests/seen_core_004e_deterministic_context.sh"
CORE_004F="$ROOT_DIR/tests/misc_root_tests/seen_program_artifacts_contract.sh"
CORE_004G="$ROOT_DIR/tests/misc_root_tests/seen_program_reproducibility_contract.sh"

fail() {
    echo "FAIL: required CI wiring: $*" >&2
    exit 1
}

[ -f "$WORKFLOW" ] && [ ! -L "$WORKFLOW" ] || fail "active workflow is missing or unsafe"
[ -x "$REQUIRED" ] && [ ! -L "$REQUIRED" ] || fail "required gate is missing or not executable"
[ -x "$CONTAINED" ] && [ ! -L "$CONTAINED" ] || fail "contained CI entrypoint is missing or not executable"
[ -x "$PROVISION" ] && [ ! -L "$PROVISION" ] || fail "CI host provisioner is missing or not executable"
[ -x "$RELEASE_INNER" ] && [ ! -L "$RELEASE_INNER" ] || fail "release entrypoint is missing or not executable"
[ -x "$RELEASE_CONTAINED" ] && [ ! -L "$RELEASE_CONTAINED" ] || fail "contained release entrypoint is missing or not executable"
[ -x "$RELEASE_CI" ] && [ ! -L "$RELEASE_CI" ] || fail "release CI verifier is missing or unsafe"
[ -f "$RELEASE_CI_CHECKER" ] && [ ! -L "$RELEASE_CI_CHECKER" ] || fail "release CI evidence checker is missing or unsafe"
[ -f "$RELEASE_TOOLCHAIN" ] && [ ! -L "$RELEASE_TOOLCHAIN" ] || fail "release toolchain checker is missing or unsafe"
[ -x "$PREPARE_RELEASE_TOOLCHAIN" ] && [ ! -L "$PREPARE_RELEASE_TOOLCHAIN" ] || fail "release toolchain preparer is missing or unsafe"
[ -f "$RELEASE_TAG_POLICY" ] && [ ! -L "$RELEASE_TAG_POLICY" ] || fail "release tag policy is missing or unsafe"
bash -n "$REQUIRED" || fail "required gate shell syntax"
bash -n "$CONTAINED" || fail "contained CI entrypoint shell syntax"
bash -n "$PROVISION" || fail "CI host provisioner shell syntax"
bash -n "$RELEASE_INNER" || fail "release entrypoint shell syntax"
bash -n "$RELEASE_CONTAINED" || fail "contained release entrypoint shell syntax"
bash -n "$RELEASE_CI" || fail "release CI verifier shell syntax"
bash -n "$PREPARE_RELEASE_TOOLCHAIN" || fail "release toolchain preparer shell syntax"
bash -n "$RELEASE_TAG_POLICY" || fail "release tag policy shell syntax"
bash -n "$PREBUILD" || fail "prebuild gate shell syntax"
bash -n "$STAGE1" || fail "Stage-1 acceptance shell syntax"
bash -n "$CORE_004E" "$CORE_004F" "$CORE_004G" ||
    fail "deterministic contract shell syntax"

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
grep -Fxq '    branches: [main]' "$WORKFLOW" ||
    fail "required CI is not scoped to main pushes"
grep -Fxq '  workflow_dispatch:' "$WORKFLOW" ||
    fail "required CI has no explicit manual fallback when push dispatch is lost"
if grep -Fq 'pull_request:' "$WORKFLOW"; then
    fail "full required CI must not run for pull requests"
fi
grep -Fxq '  required:' "$WORKFLOW" || fail "required job id is missing"
grep -Fxq '    name: required' "$WORKFLOW" || fail "required check name is missing"
grep -Eq '^[[:space:]]+uses: actions/checkout@[0-9a-f]{40}$' "$WORKFLOW" ||
    fail "checkout action is not commit-pinned"
grep -Eq '^[[:space:]]+uses: actions/setup-go@[0-9a-f]{40}$' "$WORKFLOW" ||
    fail "Go setup action is not commit-pinned"
grep -Fxq "          go-version: '1.26.5'" "$WORKFLOW" ||
    fail "required CI does not pin Go 1.26.5"
grep -Fxq '        run: scripts/run_ci_required.sh' "$WORKFLOW" ||
    fail "workflow does not invoke the required gate"
grep -Fxq '      SEEN_RELEASE_CPU_BASELINE: x86-64' "$WORKFLOW" ||
    fail "main CI does not build the portable x86-64 release compiler"
grep -Fxq '        run: scripts/prepare_release_toolchain_artifact.sh' "$WORKFLOW" ||
    fail "main CI does not exercise packaging and prepare the release toolchain"
grep -Fxq '        uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a' "$WORKFLOW" ||
    fail "main CI release toolchain upload is not commit-pinned"
grep -Fq 'name: seen-release-toolchain-${{ github.sha }}' "$WORKFLOW" ||
    fail "main CI release toolchain is not keyed by the exact commit"
grep -Fxq '        run: scripts/provision_ci_host.sh' "$WORKFLOW" ||
    fail "workflow does not invoke the CI host provisioner"
grep -Fxq '        run: scripts/verify_release_ci_run.sh' "$RELEASE_WORKFLOW" ||
    fail "release workflow does not attest successful main CI"
grep -Fxq '          ref: ${{ github.sha }}' "$RELEASE_WORKFLOW" ||
    fail "release checkout does not select the exact event commit explicitly"
if grep -Fq 'run: scripts/run_ci_required.sh' "$RELEASE_WORKFLOW"; then
    fail "release workflow redundantly reruns full certification"
fi
grep -Fq 'gh run download "$run_id"' "$RELEASE_CI" ||
    fail "release attestation does not download from the exact successful CI run"
grep -Fq '"$TOOLCHAIN_CHECKER" install' "$RELEASE_CI" ||
    fail "release attestation does not validate and install the certified toolchain"
grep -Fq 'seen_release_verify_published_tag' "$RELEASE_CI" ||
    fail "release attestation does not verify the annotated published tag"
grep -Fq 'seen_release_verify_published_tag' "$RELEASE_INNER" ||
    fail "release uploader does not reverify the annotated published tag"
grep -Fq 'tests/misc_root_tests/seen_release_upload_artifact_scope.sh' "$REQUIRED" ||
    fail "required CI omits the release mutation-safety regression"
grep -Fq 'scripts/check_qwen_contracts.py' "$REQUIRED" ||
    fail "required CI omits the Qwen schema checker"
grep -Fq 'tests/misc_root_tests/seen_qwen_prerequisites.sh' \
    "$ROOT_DIR/scripts/seen_prebuild_gates.sh" ||
    fail "serialized prebuild gates omit CPU-only Qwen prerequisites"
grep -Fq 'qwen_prerequisites.seen' \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" ||
    fail "fresh-compiler acceptance omits Qwen prerequisite contracts"
grep -Fq 'seen_json_large_object_release.sh' \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" ||
    fail "fresh-compiler acceptance omits large-object JSON release regression"
grep -Fq 'seen_safetensors_reader_ownership.sh' \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" ||
    fail "fresh-compiler acceptance omits FEL-1538 ownership regression"
grep -Fq 'seen_reactor_contract.sh' \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" ||
    fail "fresh-compiler acceptance omits reactor contract"
grep -Fq 'seen_std/tests/sync/sync-001h.seen' \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" ||
    fail "fresh-compiler acceptance omits synchronization contracts"
grep -Fq 'seen_std/examples/synchronization.seen' \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" ||
    fail "fresh-compiler acceptance omits synchronization example"
grep -Fq 'seen_parallel_for_codegen_regression.sh' "$STAGE1" ||
    fail "fresh-compiler acceptance omits parallel_for execution/codegen"
grep -Fq 'seen_parallel_for_runtime_v2.sh' "$STAGE1" ||
    fail "fresh-compiler acceptance omits parallel_for failure injection"
grep -Fq 'seen_parallel_for_fail_closed' \
    "$ROOT_DIR/tests/misc_root_tests/seen_parallel_for_codegen_regression.sh" ||
    fail "parallel_for regression omits fail-closed lowering"
grep -Fq 'ulimit -S -s 8192' "$REQUIRED" ||
    fail "required CI does not pin the hosted-runner stack limit"
grep -Fq -- '--target-cpu "$RELEASE_TARGET_CPU"' \
    "$ROOT_DIR/tests/misc_root_tests/seen_json_large_object_release.sh" ||
    fail "large-object JSON release regression does not pin the release CPU baseline"
grep -Fq -- '--target-cpu "$RELEASE_TARGET_CPU"' \
    "$ROOT_DIR/tests/misc_root_tests/seen_fs_contract.sh" ||
    fail "filesystem release regression does not pin the release CPU baseline"
for post_bootstrap_contract in \
    seen_unimported_extension_contract.sh \
    seen_time_contract.sh \
    seen_lines_linear_contract.sh; do

    contract="$ROOT_DIR/tests/misc_root_tests/$post_bootstrap_contract"
    [ "$(grep -Fc -- '--target-cpu "$RELEASE_TARGET_CPU"' "$contract")" -ge 2 ] ||
        fail "$post_bootstrap_contract does not pin both executable CPU baselines"
    [ "$(grep -Fc 'scripts/check_x86_executable_baseline.sh' "$contract")" -ge 1 ] ||
        fail "$post_bootstrap_contract lacks executable ISA auditing"
    [ "$(grep -Fc '"$BASELINE_CHECKER" "$RELEASE_TARGET_CPU"' "$contract")" -ge 2 ] ||
        fail "$post_bootstrap_contract does not audit both executables before launch"
done
if [ "$(grep -Fc -- '--target-cpu "$RELEASE_TARGET_CPU"' \
    "$ROOT_DIR/tests/misc_root_tests/seen_utf8_string_indexing.sh")" -lt 2 ]; then

    fail "tokenizer release regressions do not both pin the release CPU baseline"
fi
grep -Fq 'scripts/check_x86_executable_baseline.sh' \
    "$ROOT_DIR/tests/misc_root_tests/seen_fs_contract.sh" ||
    fail "filesystem release regression lacks executable ISA auditing"
if [ "$(grep -Fc 'scripts/check_x86_executable_baseline.sh' \
    "$ROOT_DIR/tests/misc_root_tests/seen_utf8_string_indexing.sh")" -lt 2 ]; then

    fail "tokenizer release regressions lack executable ISA auditing"
fi
grep -Fq 'seen_executable_baseline_contract.sh' "$REQUIRED" ||
    fail "required CI omits executable CPU baseline positive and negative tests"
grep -Fq 'seen_release_install_payload.sh' \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" ||
    fail "fresh-compiler acceptance omits installed release payload regression"
grep -Fq '247587' \
    "$ROOT_DIR/tests/misc_root_tests/seen_lines_linear_contract.sh" ||
    fail "lines regression does not preserve the Qwen merges corpus size"
grep -Fq '3353259' \
    "$ROOT_DIR/tests/misc_root_tests/seen_lines_linear_contract.sh" ||
    fail "lines regression does not preserve the Qwen merges byte size"
grep -Fq 'ulimit -S -s 8192' \
    "$ROOT_DIR/tests/misc_root_tests/seen_lines_linear_contract.sh" ||
    fail "lines regression does not enforce the hosted 8 MiB stack"
grep -Fq -- '--release --lto thin --no-fork' \
    "$ROOT_DIR/tests/misc_root_tests/seen_lines_linear_contract.sh" ||
    fail "lines regression omits release ThinLTO execution"
grep -Fq -- '--release --lto thin --no-fork' \
    "$ROOT_DIR/tests/misc_root_tests/seen_time_contract.sh" ||
    fail "time contract omits release ThinLTO execution"
grep -Fq 'compile "$INVALID"' \
    "$ROOT_DIR/tests/misc_root_tests/seen_unimported_extension_contract.sh" ||
    fail "unimported extension regression omits compile/check parity"
grep -Fq 'seen_core_004d_determinism_graph.sh' "$STAGE1" ||
    fail "fresh-compiler acceptance omits CORE-004D resolved determinism graph coverage"
grep -Fq 'CORE-004D_mode_parity' \
    "$ROOT_DIR/tests/misc_root_tests/seen_core_004d_determinism_graph.sh" ||
    fail "CORE-004D regression omits check/compile/run parity"
grep -Fq 'seen_program_artifacts_contract.sh' "$REQUIRED" ||
    fail "required CI omits CORE-004F artifact evidence validation"
grep -Fq 'seen_program_reproducibility_contract.sh' "$REQUIRED" ||
    fail "required CI omits CORE-004G two-builder evidence validation"
grep -Fq 'seen_project_transitive_memory_regression.sh' "$STAGE1" ||
    fail "fresh-compiler acceptance omits FEL-1547/1548 coverage"
grep -Fq 'seen_core_004e_deterministic_context.sh' "$STAGE1" ||
    fail "fresh-compiler acceptance omits CORE-004E runtime coverage"
grep -Fq 'COMPILER="$REAL_COMPILER"' "$STAGE1" ||
    fail "CORE-004E acceptance does not select the fresh compiler explicitly"
grep -Fq 'seen_program_artifacts_contract.sh' "$PREPARE_RELEASE_TOOLCHAIN" ||
    fail "release preparation omits CORE-004F evidence validation"
grep -Fq 'seen_program_reproducibility_contract.sh' "$PREPARE_RELEASE_TOOLCHAIN" ||
    fail "release preparation omits CORE-004G evidence validation"
[ "$(grep -Fc 'seen_core_004e_deterministic_context.sh' "$REQUIRED")" -eq 1 ] ||
    fail "required CI must syntax-check CORE-004E once and execute it only through Stage-1 acceptance"
grep -Fq '248044' \
    "$ROOT_DIR/tests/misc_root_tests/seen_json_large_object_release.sh" ||
    fail "large-object regression does not preserve the production vocabulary size"
grep -Fq '@llvm.stacksave()' \
    "$ROOT_DIR/compiler_seen/src/codegen/ir_method_array_mutator_emit.seen" ||
    fail "generic Array push lowering lacks bounded scratch lifetime"
grep -Fq '@llvm.stackrestore(ptr' \
    "$ROOT_DIR/compiler_seen/src/codegen/ir_assignment_gen.seen" ||
    fail "generic Array assignment lowering lacks bounded scratch lifetime"
grep -Fq 'seen_arr_set_i64' \
    "$ROOT_DIR/compiler_seen/tests/array_bool_push_codegen.seen" ||
    fail "Boolean Array assignment direct-lowering contract is missing"
grep -Fq 'production_vocab_contract.seen' \
    "$ROOT_DIR/tests/misc_root_tests/seen_utf8_string_indexing.sh" ||
    fail "tokenizer verification omits the production vocabulary contract"
grep -Fq 'projects/seen_ml/qwen38/tests/prerequisites.seen' \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" ||
    fail "fresh-compiler acceptance omits Qwen project contracts"
for required_tag_ref in 'refs/heads/main' '"$tag_ref"' '"$tag_ref^{}"'; do
    grep -Fq "$required_tag_ref" "$RELEASE_TAG_POLICY" ||
        fail "release tag policy omits $required_tag_ref"
done
if grep -Fq -- '--clobber' "$RELEASE_INNER"; then
    fail "release uploader retains an asset-overwrite path"
fi
grep -Fq -- '--verify-tag' "$RELEASE_INNER" ||
    fail "release creation can implicitly create a missing tag"
if grep -Fq 'git -C "$ROOT_DIR" push' "$RELEASE_INNER" ||
    grep -Fq 'git -C "$ROOT_DIR" tag' "$RELEASE_INNER"; then

    fail "release uploader retains a tag-publication path"
fi

[ "$(grep -Ec '(^|[[:space:]])sudo([[:space:]]|$)' "$PROVISION")" -eq 3 ] ||
    fail "CI host provisioner has an unexpected privileged-command count"
grep -Fxq 'sudo -n apt-get update' "$PROVISION" ||
    fail "CI host provisioner omits package-index setup"
grep -Fq 'bubblewrap ripgrep libvulkan-dev llvm-20 clang-20 lld-20' \
    "$PROVISION" || fail "CI host provisioner omits pinned LLVM 20 packages"
grep -Fq 'libclang-rt-20-dev' "$PROVISION" ||
    fail "CI host provisioner omits pinned LLVM 20 instrumentation runtimes"
grep -Fq 'versioned_tool="/usr/bin/${llvm_tool}-20"' "$PROVISION" ||
    fail "CI host provisioner omits pinned LLVM 20 tool resolution"
grep -Fq 'getelementptr inbounds nuw i64' "$PROVISION" ||
    fail "CI host provisioner omits the immediate Seen IR dialect probe"
grep -Fq -- '-fprofile-instr-generate' "$PROVISION" ||
    fail "CI host provisioner omits the coverage runtime link probe"
grep -Fq -- '-fsanitize=undefined' "$PROVISION" ||
    fail "CI host provisioner omits the sanitizer runtime link probe"
grep -Fq 'printf '\''%s\n'\'' "$llvm_root" >> "$GITHUB_PATH"' "$PROVISION" ||
    fail "CI host provisioner does not publish the pinned toolchain"
grep -Fxq 'sudo -n /usr/sbin/apparmor_parser --replace "$profile"' "$PROVISION" ||
    fail "CI host provisioner omits scoped AppArmor loading"
grep -Fq '/usr/bin/bwrap flags=(default_allow) {' "$PROVISION" ||
    fail "CI host provisioner omits the path-scoped Bubblewrap profile"
grep -Fq "'  userns,'" "$PROVISION" ||
    fail "CI host provisioner omits the user-namespace grant"
grep -Fq 'mapped_identity=$(/usr/bin/bwrap' "$PROVISION" ||
    fail "CI host provisioner omits live namespace read-back"
grep -Fq "/usr/bin/nm -D --undefined-only \"\$frozen\"" "$PROVISION" ||
    fail "CI host provisioner does not prove the bootstrap bridge is symbol-free"
grep -Fq -- '-Wl,-soname,libSDL3.so.0' "$PROVISION" ||
    fail "CI host provisioner omits the bounded bootstrap loader bridge"
grep -Fq "printf 'LD_LIBRARY_PATH=%s\\n' \"\$compat_root\" >> \"\$GITHUB_ENV\"" \
    "$PROVISION" || fail "CI host provisioner does not publish the loader bridge"
grep -Fq 'linkCmd = linkCmd + " -Wl,--as-needed "' \
    "$ROOT_DIR/compiler_seen/src/main_compiler.seen" ||
    fail "Linux compiler links do not prune unused optional libraries"

for required_command in \
    'python3 scripts/check_compatibility_manifest.py' \
    'tests/misc_root_tests/seen_import_graph_contract.sh' \
    'tests/misc_root_tests/seen_global_initialization_contract.sh' \
    'tests/misc_root_tests/seen_production_ir_policy_contract.sh' \
    'tests/misc_root_tests/seen_production_source_policy_contract.sh' \
    'tests/misc_root_tests/seen_machine_diagnostic_contract.sh' \
    'tests/misc_root_tests/seen_build_instrumentation_contract.sh' \
    'tests/misc_root_tests/seen_release_optimization_contract.sh' \
    'tests/misc_root_tests/seen_test_discovery_contract.sh' \
    'python3 scripts/check_ci_workflows.py' \
    'python3 scripts/check_ci_containment.py' \
    'tests/misc_root_tests/seen_compatibility_manifest_contract.sh' \
    'tests/misc_root_tests/seen_native_boundaries_ledger.sh' \
    'tests/misc_root_tests/seen_native_inventory_gate.sh' \
    'tests/misc_root_tests/seen_sync_contract.sh' \
    'tests/misc_root_tests/seen_sync_native_abi.sh' \
    'tests/misc_root_tests/seen_cpu_benchmark_clock_contract.sh' \
    'tests/misc_root_tests/seen_ci_workflow_contract.sh' \
    'tests/misc_root_tests/seen_ci_containment_contract.sh' \
    'tests/misc_root_tests/seen_gate0_certification_contract.sh' \
    'scripts/certify_gate0_clean_checkout.sh' \
    'tests/misc_root_tests/seen_program_artifacts_contract.sh' \
    'tests/misc_root_tests/seen_program_reproducibility_contract.sh' \
    'git diff --check'; do

    grep -Fq "$required_command" "$REQUIRED" ||
        fail "required gate omitted $required_command"
done

for fresh_compiler_contract in \
    seen_unimported_extension_contract.sh \
    seen_core_004d_determinism_graph.sh \
    seen_time_contract.sh \
    seen_lines_linear_contract.sh \
    seen_fs_contract.sh \
    seen_tokenizers_a.sh; do

    grep -Fq "$fresh_compiler_contract" "$STAGE1" ||
        fail "fresh-compiler acceptance omits $fresh_compiler_contract"
    if grep -Fq "$fresh_compiler_contract" "$REQUIRED"; then
        fail "$fresh_compiler_contract escapes the attested Stage-1 artifact scope"
    fi
done
stabilization_line=$(grep -nF 'seen_core_004d_determinism_graph.sh' "$STAGE1" |
    cut -d: -f1)
acceptance_corpus_line=$(grep -nF 'run_fixture type-ref ' "$STAGE1" | cut -d: -f1)
case "$stabilization_line:$acceptance_corpus_line" in
    *[!0-9:]*|:*|*::*) fail "fresh-compiler fail-fast ordering is ambiguous" ;;
esac
[ "$stabilization_line" -lt "$acceptance_corpus_line" ] ||
    fail "release-blocking fresh-compiler contracts must precede the broad acceptance corpus"
grep -Fq 'PACKAGE_CLIENT="${SEEN_PACKAGE_CLIENT:-$ROOT_DIR/compiler_seen/target/seen-pkg}"' \
    "$ROOT_DIR/tests/misc_root_tests/seen_tokenizers_a.sh" ||
    fail "tokenizer gate does not consume the verified rebuild package client"
if grep -Fq 'tools/seen-pkg/bin/seen-pkg' \
    "$ROOT_DIR/tests/misc_root_tests/seen_tokenizers_a.sh"; then
    fail "tokenizer gate depends on a local-only package client"
fi

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
grep -Fq 'scripts/run_in_hard_memory_scope.sh' "$RELEASE_CONTAINED" ||
    fail "contained release entrypoint does not enter the hard scope"
grep -Fq 'SEEN_RELEASE_CONTAINMENT_IN_SCOPE=1' "$RELEASE_CONTAINED" ||
    fail "contained release entrypoint does not identify the inner release"
grep -Fq 'SEEN_CI_CONTAINMENT_IN_SCOPE=1' "$RELEASE_CONTAINED" ||
    fail "contained release entrypoint does not suppress nested aggregate guards"
grep -Fq 'SEEN_PACKAGE_JOBS=1' "$RELEASE_CONTAINED" ||
    fail "contained release entrypoint does not serialize package jobs"
grep -Fq 'SEEN_NO_FORK=1' "$RELEASE_CONTAINED" ||
    fail "contained release entrypoint does not disable compiler forking"
grep -Fq 'run_in_hard_memory_scope.sh" --verify-only' "$RELEASE_INNER" ||
    fail "inner release does not re-verify the live hard scope"
grep -Fq 'getOrDefault("SEEN_NO_FORK", "0") == "1"' \
    "$ROOT_DIR/compiler_seen/src/main_compiler.seen" ||
    fail "compiler does not honor the contained no-fork release contract"

if grep -En 'compiler_seen/target/seen|(^|[[:space:]])sudo([[:space:]]|$)|/tmp/' \
    "$WORKFLOW" "$REQUIRED" "$CONTAINED"; then

    fail "required CI unexpectedly invokes an unmediated compiler, privilege escalation, or host temporary path"
fi
if grep -En 'compiler_seen/target/seen|/tmp/' "$PROVISION"; then
    fail "CI host provisioner invokes an unmediated compiler or host temporary path"
fi

echo "PASS: required CI wiring"
