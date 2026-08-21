#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OPT_VMEM_KB="${SEEN_OPT_VMEM_KB:-2097152}"
ARTIFACT_ROOT_SCRIPT="$SCRIPT_DIR/artifact_root.sh"
HARD_SCOPE_WRAPPER="$SCRIPT_DIR/run_in_hard_memory_scope.sh"
SERIALIZER_VERIFY="$SCRIPT_DIR/verify_fork_serializer.sh"
BOUNDED_TOOLCHAIN_PREPARE="$SCRIPT_DIR/prepare_bounded_toolchain.sh"
ARTIFACT_PREFLIGHT_ONLY=0
PREBUILD_WORK_OWNED=0

case "$#" in
    0) ;;
    1)
        if [ "$1" = "--artifact-preflight" ]; then
            ARTIFACT_PREFLIGHT_ONLY=1
        else
            echo "ERROR: unknown prebuild gate option: $1" >&2
            exit 1
        fi
        ;;
    *)
        echo "ERROR: seen_prebuild_gates.sh accepts only --artifact-preflight" >&2
        exit 1
        ;;
esac

if [ ! -f "$ARTIFACT_ROOT_SCRIPT" ]; then
    echo "ERROR: missing artifact-root helper: $ARTIFACT_ROOT_SCRIPT" >&2
    exit 1
fi
# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_ROOT_SCRIPT"
seen_artifact_root_init "$REPO_ROOT"
PREBUILD_ARTIFACT_SCOPE=$(seen_artifact_scope_init prebuild-gates)
if [ "${SEEN_ARTIFACT_NAMESPACE_ACTIVE:-0}" = "1" ]; then
    PREBUILD_WORK_ROOT=${SEEN_REBUILD_WORK_ROOT:-}
    inherited_project_artifact_root=${PROJECT_ARTIFACT_ROOT:-}
    case "$PREBUILD_WORK_ROOT" in
        "$inherited_project_artifact_root"/safe-rebuild/run.*|\
            "$inherited_project_artifact_root"/safe-rebuild/preflight) ;;
        *)
            echo "ERROR: invalid inherited rebuild artifact directory: $PREBUILD_WORK_ROOT" >&2
            exit 1
            ;;
    esac
    if [ "$PREBUILD_WORK_ROOT" != "$SEEN_ARTIFACT_ROOT" ]; then
        echo "ERROR: inherited rebuild work root does not match SEEN_ARTIFACT_ROOT" >&2
        exit 1
    fi
elif [ "${SEEN_PREFLIGHT_NAMESPACE_ACTIVE:-0}" = "1" ]; then
    PREBUILD_WORK_ROOT=${SEEN_PREFLIGHT_WORK_ROOT:-}
    case "$PREBUILD_WORK_ROOT" in
        "$PREBUILD_ARTIFACT_SCOPE"/run.*) ;;
        *)
            echo "ERROR: invalid inherited prebuild artifact directory: $PREBUILD_WORK_ROOT" >&2
            exit 1
            ;;
    esac
    if [ ! -d "$PREBUILD_WORK_ROOT" ] || [ -L "$PREBUILD_WORK_ROOT" ]; then
        echo "ERROR: inherited prebuild artifact directory is unsafe: $PREBUILD_WORK_ROOT" >&2
        exit 1
    fi
    if [ "${SEEN_PREFLIGHT_OWNS_WORK_ROOT:-0}" = "1" ]; then
        PREBUILD_WORK_OWNED=1
    fi
else
    PREBUILD_WORK_ROOT=$(seen_artifact_mktemp_dir "$PREBUILD_ARTIFACT_SCOPE" run)
    PREBUILD_WORK_OWNED=1
fi

case "$PREBUILD_WORK_ROOT" in
    "$REPO_ROOT"/*) prebuild_work_relative=${PREBUILD_WORK_ROOT#"$REPO_ROOT"/} ;;
    *)
        echo "ERROR: prebuild artifact directory escaped the repository: $PREBUILD_WORK_ROOT" >&2
        exit 1
        ;;
esac
seen_artifact_assert_safe_relative_path "$prebuild_work_relative"
seen_artifact_assert_no_symlink_components "$REPO_ROOT" "$prebuild_work_relative"
canonical_prebuild_work_root=$(seen_artifact_canonical_dir "$PREBUILD_WORK_ROOT") || {
    echo "ERROR: could not resolve prebuild artifact directory: $PREBUILD_WORK_ROOT" >&2
    exit 1
}
if [ "$canonical_prebuild_work_root" != "$PREBUILD_WORK_ROOT" ]; then
    echo "ERROR: inherited prebuild artifact directory was not canonical: $PREBUILD_WORK_ROOT" >&2
    exit 1
fi

if [ "$(uname -s)" = "Linux" ] &&
    { [ "${SEEN_ARTIFACT_NAMESPACE_ACTIVE:-0}" = "1" ] ||
        [ "${SEEN_PREFLIGHT_NAMESPACE_ACTIVE:-0}" = "1" ]; }; then

    namespace_tmp_identity=$(stat -c '%d:%i' /tmp 2>/dev/null || true)
    work_root_identity=$(stat -c '%d:%i' "$PREBUILD_WORK_ROOT" 2>/dev/null || true)
    if [ -z "$namespace_tmp_identity" ] ||
        [ "$namespace_tmp_identity" != "$work_root_identity" ]; then

        echo "ERROR: prebuild artifact namespace validation failed." >&2
        exit 1
    fi
fi

TMPDIR="$PREBUILD_WORK_ROOT/tool-tmp"
if [ -L "$TMPDIR" ]; then
    echo "ERROR: prebuild tool temp directory is a symbolic link: $TMPDIR" >&2
    exit 1
fi
mkdir -p -- "$TMPDIR"
export TMPDIR SEEN_ARTIFACT_ROOT

prebuild_cleanup() {
    local status=$?
    if [ "$status" -eq 0 ] && [ "$PREBUILD_WORK_OWNED" = "1" ]; then
        case "$PREBUILD_WORK_ROOT" in
            "$PREBUILD_ARTIFACT_SCOPE"/run.*)
                if [ -d "$PREBUILD_WORK_ROOT" ] && [ ! -L "$PREBUILD_WORK_ROOT" ] &&
                    [ "$(dirname -- "$PREBUILD_WORK_ROOT")" = "$PREBUILD_ARTIFACT_SCOPE" ]; then
                    rm -rf -- "$PREBUILD_WORK_ROOT"
                fi
                ;;
        esac
    fi
    return "$status"
}
trap prebuild_cleanup EXIT

# Several legacy prebuild fixtures still contain absolute temporary paths. Map
# them onto this project-local run directory rather than writing to host /tmp.
if [ "${SEEN_ARTIFACT_NAMESPACE_ACTIVE:-0}" != "1" ] &&
    [ "${SEEN_PREFLIGHT_NAMESPACE_ACTIVE:-0}" != "1" ]; then
    prebuild_host_os=$(uname -s)
    if [ "$prebuild_host_os" = "Linux" ]; then
        if ! command -v bwrap >/dev/null 2>&1; then
            echo "ERROR: project-local prebuild artifacts require bwrap on Linux." >&2
            exit 1
        fi
        if ! bwrap --die-with-parent --bind / / --dev-bind /dev /dev \
            --proc /proc --ro-bind /sys /sys \
            --bind "$PREBUILD_WORK_ROOT" /tmp -- true; then
            echo "ERROR: could not map the project prebuild directory onto /tmp." >&2
            exit 1
        fi
        export SEEN_PREFLIGHT_NAMESPACE_ACTIVE=1
        export SEEN_PREFLIGHT_WORK_ROOT="$PREBUILD_WORK_ROOT"
        export SEEN_PREFLIGHT_OWNS_WORK_ROOT="$PREBUILD_WORK_OWNED"
        exec bwrap --die-with-parent --bind / / --dev-bind /dev /dev \
            --proc /proc --ro-bind /sys /sys \
            --bind "$PREBUILD_WORK_ROOT" /tmp -- \
            "$SCRIPT_DIR/seen_prebuild_gates.sh" "$@"
    elif [ "${SEEN_ALLOW_SYSTEM_TMP:-0}" != "1" ]; then
        echo "ERROR: legacy prebuild fixtures cannot redirect temporary paths on $prebuild_host_os." >&2
        echo "Set SEEN_ALLOW_SYSTEM_TMP=1 only if host temporary-file use is acceptable." >&2
        exit 1
    fi
fi

if [ "$ARTIFACT_PREFLIGHT_ONLY" = "1" ]; then
    echo "Project artifact root: $SEEN_ARTIFACT_ROOT"
    echo "Prebuild artifact directory: $PREBUILD_WORK_ROOT"
    if [ "$(uname -s)" = "Linux" ]; then
        echo "Legacy-fixture temporary mapping: project-local"
    else
        echo "Legacy-fixture temporary mapping: legacy host temporary directory allowed"
    fi
    exit 0
fi

if [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != "1" ] &&
    [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" != "1" ]; then
    [ -x "$HARD_SCOPE_WRAPPER" ] || {
        echo "ERROR: missing hard-memory-scope wrapper: $HARD_SCOPE_WRAPPER" >&2
        exit 1
    }
    exec "$HARD_SCOPE_WRAPPER" --label "Seen prebuild gates" -- "$0"
fi
SEEN_HARD_MEMORY_SCOPE_ACTIVE=1
export SEEN_HARD_MEMORY_SCOPE_ACTIVE
"$HARD_SCOPE_WRAPPER" --label "Seen prebuild gates read-back" \
    --verify-only --
if ! bash "$SERIALIZER_VERIFY" "${SEEN_FORK_SERIALIZER_SO:-}" \
    "${SEEN_FORK_SERIALIZER_ATTESTATION:-}" "$SEEN_ARTIFACT_ROOT" \
    "${SEEN_MEMORY_GUARD_SCOPE_UNIT:-}" >/dev/null; then

    echo "ERROR: prebuild gates require the scope-attested fork serializer produced by safe_rebuild" >&2
    exit 126
fi
BOUNDED_TOOLCHAIN_DIR=$(bash "$BOUNDED_TOOLCHAIN_PREPARE" "$SEEN_ARTIFACT_ROOT") ||
    exit 126
PATH="$BOUNDED_TOOLCHAIN_DIR:$PATH"
export PATH SEEN_BOUNDED_TOOLCHAIN_DIR="$BOUNDED_TOOLCHAIN_DIR"

require_cmd() {
    local name="$1"
    if ! command -v "$name" >/dev/null 2>&1; then
        echo "ERROR: required prebuild gate command not found: $name" >&2
        exit 1
    fi
}

run_with_opt_cap() {
    (
        if ! ulimit -S -v "$OPT_VMEM_KB" 2>/dev/null; then
            echo "ERROR: could not apply optimizer virtual-memory cap (${OPT_VMEM_KB} KiB)" >&2
            exit 126
        fi
        active_opt_vmem=$(ulimit -S -v 2>/dev/null || true)
        case "$active_opt_vmem" in
            ''|*[!0-9]*)
                echo "ERROR: could not read back optimizer virtual-memory cap" >&2
                exit 126
                ;;
        esac
        if [ "$active_opt_vmem" -gt "$OPT_VMEM_KB" ]; then
            echo "ERROR: optimizer virtual-memory cap read-back exceeds ${OPT_VMEM_KB} KiB" >&2
            exit 126
        fi
        "$@"
    )
}

sweep_saved_ll_dir() {
    local source_dir="${SEEN_PREFLIGHT_LL_DIR:-}"
    if [ -z "$source_dir" ]; then
        return 0
    fi
    if [ ! -d "$source_dir" ]; then
        echo "ERROR: SEEN_PREFLIGHT_LL_DIR does not exist: $source_dir" >&2
        exit 1
    fi

    local work_dir
    work_dir="$(mktemp -d "$PREBUILD_WORK_ROOT/seen-preflight-ll.XXXXXX")"

    local count=0
    local ll
    for ll in "$source_dir"/seen_module_*.ll; do
        [ -f "$ll" ] || continue
        [[ "$ll" == *.opt.ll ]] && continue
        count=$((count + 1))
        local copy="$work_dir/$(basename "$ll")"
        cp "$ll" "$copy"
    done

    if [ "$count" -eq 0 ]; then
        echo "ERROR: no seen_module_*.ll files found in SEEN_PREFLIGHT_LL_DIR=$source_dir" >&2
        rm -rf "$work_dir"
        exit 1
    fi
    # CORE-003C: saved current-compiler IR is verified byte-for-byte as emitted.
    # Never run the frozen compatibility adapter over production artifacts.
    run_with_opt_cap python3 "$SCRIPT_DIR/verify_ir_call_shapes.py" "$work_dir"
    for ll in "$work_dir"/seen_module_*.ll; do
        [ -f "$ll" ] || continue
        run_with_opt_cap llvm-as "$ll" -o /dev/null
    done
    echo "PASS: preflight verified $count unmodified production .ll file(s)"
    rm -rf "$work_dir"
}

cd "$REPO_ROOT"

require_cmd python3
require_cmd bash
require_cmd llvm-as
require_cmd opt

echo "Prebuild gates: Python and shell syntax..."
python3 -m py_compile "$SCRIPT_DIR/fix_ir.py" \
    "$SCRIPT_DIR/check_production_ir_policy.py" \
    "$SCRIPT_DIR/benchmark_production_ir_policy.py" \
    "$SCRIPT_DIR/check_production_source_policy.py" \
    "$SCRIPT_DIR/benchmark_production_source_policy.py" \
    "$SCRIPT_DIR/check_codegen_abi_boundaries.py" \
    "$SCRIPT_DIR/verify_ir_call_shapes.py" \
    "$REPO_ROOT/tests/package_registry_contracts.py"
bash -n "$SCRIPT_DIR/artifact_root.sh" \
    "$SCRIPT_DIR/memory_guard.sh" \
    "$SCRIPT_DIR/prepare_bounded_toolchain.sh" \
    "$SCRIPT_DIR/rebuild_builder_applicability.sh" \
    "$SCRIPT_DIR/rebuild_builder_capability.sh" \
    "$SCRIPT_DIR/rebuild_builder_selection.sh" \
    "$SCRIPT_DIR/run_in_hard_memory_scope.sh" \
    "$SCRIPT_DIR/serial_auxiliary_env.sh" \
    "$SCRIPT_DIR/run_with_project_artifacts.sh" \
    "$SCRIPT_DIR/build_package_client.sh" \
    "$SCRIPT_DIR/run_all_tests.sh" \
    "$SCRIPT_DIR/run_attested_seen.sh" \
    "$SCRIPT_DIR/run_capped_regression.sh" \
    "$SCRIPT_DIR/safe_rebuild.sh" \
    "$SCRIPT_DIR/seen_stage1_acceptance.sh" \
    "$SCRIPT_DIR/verify_fork_serializer.sh" \
    "$SCRIPT_DIR/recovery_opt.sh" \
    "$SCRIPT_DIR/seen_prebuild_gates.sh" \
    "$REPO_ROOT/installer/test/integration-test.sh" \
    "$REPO_ROOT/installer/test/test-runner.sh" \
    "$REPO_ROOT/installer/linux/build-deb.sh" \
    "$REPO_ROOT/installer/linux/build-rpm.sh" \
    "$REPO_ROOT/installer/linux/build-appimage.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_artifact_root.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_compiler_artifact_root.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_fix_ir_stage2_patterns.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_production_ir_policy_contract.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_production_source_policy_contract.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_codegen_abi_preflight.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_ir_call_shape_preflight.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_selfhosted_abi_smoke.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_cli_surface.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_cli_array_bool_bootstrap.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_lexer_interface_token_type.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_pkg_prebuild_operand_resolution.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_atomic_text_io.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_capped_regression_wiring.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_memory_guard_fail_closed.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_low_task_helper_serialization.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_rebuild_builder_selection.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_perf_gate_hardening.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_linux_installer_handshake.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_editor_feature_parity.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_extern_runtime_declaration_dedup.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_test_runner_contract.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_assertions_snapshot_contract.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_fixture_isolation_contract.sh" \
    "$REPO_ROOT/tests/e2e_multilang/run_all_e2e.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_pkg_local_registry.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_pkg_scoped_identity.sh"

echo "Prebuild gates: Stage-1 acceptance wiring..."
grep -Fq 'run_stage1_acceptance_checks "$compiler_path" "$REBUILD_TIER"' \
    "$SCRIPT_DIR/safe_rebuild.sh"
grep -Fq 'run_stage1_acceptance_checks "$VERIFIED" verify' \
    "$SCRIPT_DIR/safe_rebuild.sh"
for fixture in type_ref.seen lexical_semantic.seen \
    declaration_frontend_smoke.seen generic_parameter_metadata.seen \
    language_pack_validation.seen lsp_formatter.seen \
    nullable_option_codegen.seen nullable_class_primitive_runtime.seen \
    array_bool_push_codegen.seen; do
    grep -Fq "$fixture" "$SCRIPT_DIR/seen_stage1_acceptance.sh"
done
grep -Fq 'seen_extern_runtime_declaration_dedup.sh' \
    "$SCRIPT_DIR/seen_stage1_acceptance.sh"
grep -Fq 'seen_semantic_foundation.sh' \
    "$SCRIPT_DIR/seen_stage1_acceptance.sh"
grep -Fq 'lsp_formatter_jsonrpc.py' "$SCRIPT_DIR/seen_stage1_acceptance.sh"
grep -Fq 'seen_editor_feature_parity.sh' "$SCRIPT_DIR/seen_stage1_acceptance.sh"
grep -Fq 'seen_atomic_text_io.sh' "$SCRIPT_DIR/seen_stage1_acceptance.sh"
grep -Fq 'seen_linux_installer_handshake.sh' \
    "$SCRIPT_DIR/seen_stage1_acceptance.sh"
grep -Fq 'run_all_e2e.sh' "$SCRIPT_DIR/seen_stage1_acceptance.sh"
grep -Fq 'SEEN_PROJECT_ARTIFACT_WRAPPER' \
    "$REPO_ROOT/tests/e2e_multilang/run_all_e2e.sh"
cli_expected_separator_count=$(grep -Fc -- 'grep -Fq -- "$expected"' \
    "$REPO_ROOT/tests/misc_root_tests/seen_cli_surface.sh")
if [ "$cli_expected_separator_count" -ne 3 ]; then
    echo "ERROR: CLI surface assertions must protect dash-prefixed expected text with grep --" >&2
    exit 1
fi
if rg -n 'binary="/tmp/|rm -rf /tmp/|rm -f /tmp/' \
    "$REPO_ROOT/tests/e2e_multilang/run_all_e2e.sh"; then
    echo "ERROR: multilingual E2E still writes or removes host temporary paths" >&2
    exit 1
fi

echo "Prebuild gates: CLI bootstrap array-width contract..."
bash "$REPO_ROOT/tests/misc_root_tests/seen_cli_array_bool_bootstrap.sh"

echo "Prebuild gates: canonical test runner contract..."
bash "$REPO_ROOT/tests/misc_root_tests/seen_test_runner_contract.sh"

echo "Prebuild gates: assertion and snapshot contract..."
bash "$REPO_ROOT/tests/misc_root_tests/seen_assertions_snapshot_contract.sh"

echo "Prebuild gates: deterministic fixture isolation contract..."
bash "$REPO_ROOT/tests/misc_root_tests/seen_fixture_isolation_contract.sh"

echo "Prebuild gates: lexer interface token type contract..."
bash "$REPO_ROOT/tests/misc_root_tests/seen_lexer_interface_token_type.sh"

echo "Prebuild gates: explicit package manifest resolution contract..."
bash "$REPO_ROOT/tests/misc_root_tests/seen_pkg_prebuild_operand_resolution.sh"

echo "Prebuild gates: fail-closed memory containment..."
bash "$REPO_ROOT/tests/misc_root_tests/seen_capped_regression_wiring.sh"
bash "$REPO_ROOT/tests/misc_root_tests/seen_memory_guard_fail_closed.sh"
bash "$REPO_ROOT/tests/misc_root_tests/seen_low_task_helper_serialization.sh"
bash "$REPO_ROOT/tests/misc_root_tests/seen_rebuild_builder_selection.sh"
bash "$REPO_ROOT/tests/misc_root_tests/seen_perf_gate_hardening.sh"

echo "Prebuild gates: package registry draft contracts..."
PYTHONDONTWRITEBYTECODE=1 python3 \
    "$REPO_ROOT/tests/package_registry_contracts.py"

echo "Prebuild gates: project-local compiler artifact paths..."
SEEN_ARTIFACT_STATIC_ONLY=1 \
    bash "$REPO_ROOT/tests/misc_root_tests/seen_compiler_artifact_root.sh"

if [ "${SEEN_SKIP_CODEGEN_ABI_PREFLIGHT:-0}" != "1" ]; then
    echo "Prebuild gates: codegen ABI/import/cycle checks..."
    python3 "$SCRIPT_DIR/check_codegen_abi_boundaries.py" "$REPO_ROOT"
else
    echo "Prebuild gates: codegen ABI checks skipped by SEEN_SKIP_CODEGEN_ABI_PREFLIGHT=1"
fi

echo "Prebuild gates: codegen ABI regression fixtures..."
bash "$REPO_ROOT/tests/misc_root_tests/seen_codegen_abi_preflight.sh"

echo "Prebuild gates: unmodified production IR policy..."
bash "$REPO_ROOT/tests/misc_root_tests/seen_production_ir_policy_contract.sh"

echo "Prebuild gates: unmodified production source policy..."
bash "$REPO_ROOT/tests/misc_root_tests/seen_production_source_policy_contract.sh"

echo "Prebuild gates: frozen Stage-1 IR compatibility patterns under ${OPT_VMEM_KB} KiB cap..."
run_with_opt_cap bash "$REPO_ROOT/tests/misc_root_tests/seen_fix_ir_stage2_patterns.sh"

echo "Prebuild gates: IR call shape verifier..."
run_with_opt_cap bash "$REPO_ROOT/tests/misc_root_tests/seen_ir_call_shape_preflight.sh"

if [ "${SEEN_SKIP_SELFHOSTED_ABI_SMOKE:-0}" != "1" ]; then
    echo "Prebuild gates: self-hosted ABI smoke fixture..."
    SEEN_SELFHOSTED_ABI_VMEM_KB="${SEEN_SELFHOSTED_ABI_VMEM_KB:-${SEEN_MAIN_VMEM_KB:-2097152}}" \
        bash "$REPO_ROOT/tests/misc_root_tests/seen_selfhosted_abi_smoke.sh"
else
    echo "Prebuild gates: self-hosted ABI smoke skipped by SEEN_SKIP_SELFHOSTED_ABI_SMOKE=1"
fi

sweep_saved_ll_dir

echo "PASS: prebuild gates"
