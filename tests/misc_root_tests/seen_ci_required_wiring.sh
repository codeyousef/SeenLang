#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
WORKFLOW="$ROOT_DIR/.github/workflows/ci.yml"
REQUIRED="$ROOT_DIR/scripts/ci_required.sh"
CONTAINED="$ROOT_DIR/scripts/run_ci_required.sh"
PROVISION="$ROOT_DIR/scripts/provision_ci_host.sh"

fail() {
    echo "FAIL: required CI wiring: $*" >&2
    exit 1
}

[ -f "$WORKFLOW" ] && [ ! -L "$WORKFLOW" ] || fail "active workflow is missing or unsafe"
[ -x "$REQUIRED" ] && [ ! -L "$REQUIRED" ] || fail "required gate is missing or not executable"
[ -x "$CONTAINED" ] && [ ! -L "$CONTAINED" ] || fail "contained CI entrypoint is missing or not executable"
[ -x "$PROVISION" ] && [ ! -L "$PROVISION" ] || fail "CI host provisioner is missing or not executable"
bash -n "$REQUIRED" || fail "required gate shell syntax"
bash -n "$CONTAINED" || fail "contained CI entrypoint shell syntax"
bash -n "$PROVISION" || fail "CI host provisioner shell syntax"

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
grep -Eq '^[[:space:]]+uses: actions/setup-go@[0-9a-f]{40}$' "$WORKFLOW" ||
    fail "Go setup action is not commit-pinned"
grep -Fxq "          go-version: '1.26.5'" "$WORKFLOW" ||
    fail "required CI does not pin Go 1.26.5"
grep -Fxq '        run: scripts/run_ci_required.sh' "$WORKFLOW" ||
    fail "workflow does not invoke the required gate"
grep -Fxq '        run: scripts/provision_ci_host.sh' "$WORKFLOW" ||
    fail "workflow does not invoke the CI host provisioner"

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
    'tests/misc_root_tests/seen_ci_workflow_contract.sh' \
    'tests/misc_root_tests/seen_ci_containment_contract.sh' \
    'tests/misc_root_tests/seen_gate0_certification_contract.sh' \
    'scripts/certify_gate0_clean_checkout.sh' \
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

if grep -En 'compiler_seen/target/seen|(^|[[:space:]])sudo([[:space:]]|$)|/tmp/' \
    "$WORKFLOW" "$REQUIRED" "$CONTAINED"; then

    fail "required CI unexpectedly invokes an unmediated compiler, privilege escalation, or host temporary path"
fi
if grep -En 'compiler_seen/target/seen|/tmp/' "$PROVISION"; then
    fail "CI host provisioner invokes an unmediated compiler or host temporary path"
fi

echo "PASS: required CI wiring"
