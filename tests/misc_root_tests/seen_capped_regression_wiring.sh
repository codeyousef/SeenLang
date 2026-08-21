#!/usr/bin/env bash
# Static/no-build checks for compiler-capable regression containment.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
RUNNER="$ROOT_DIR/scripts/run_attested_seen.sh"
STAGE1_ACCEPTANCE="$ROOT_DIR/scripts/seen_stage1_acceptance.sh"
ATOMIC_TEXT_IO="$ROOT_DIR/tests/misc_root_tests/seen_atomic_text_io.sh"
ARTIFACT_HELPER="$ROOT_DIR/scripts/artifact_root.sh"
SERIAL_AUXILIARY="$ROOT_DIR/scripts/serial_auxiliary_env.sh"
CAPABILITY_CALLERS=(
    "$ROOT_DIR/scripts/run_capped_regression.sh"
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh"
    "$ROOT_DIR/scripts/run_all_tests.sh"
    "$ROOT_DIR/tests/e2e_multilang/run_all_e2e.sh"
    "$ROOT_DIR/scripts/perf_gate.sh"
    "$ROOT_DIR/scripts/run_production_benchmarks.sh"
    "$ROOT_DIR/scripts/safe_rebuild.sh"
)
TARGETS=(
    "$ROOT_DIR/tests/misc_root_tests/seen_build_modules_struct_arg.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_checked_allocation_codegen.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_companion_struct_literal_arg.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_gpu_runtime_wrapper_link.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_import_c_anonymous_records.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_import_c_array_fallbacks.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_import_c_bitfields.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_import_c_bitfields_64.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_import_c_bootstrap_module.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_import_c_enums.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_import_c_real_headers.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_import_c_typedefs.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_import_c_unions.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_pkg_prebuild_artifact_string_fields.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_pkg_prebuild_artifact_string_helpers.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_pkg_prebuild_artifact_symbols.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_pkg_prebuild_artifact_unqualified_string_helpers.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_pkg_prebuild_concurrent_temp_paths.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_pkg_local_registry.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_pkg_scoped_identity.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_package_run_preparation.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_return_class_get_helper.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_selfhosted_abi_smoke.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_string_equality_return_regression.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_trainer_blocker_regressions.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_unresolved_struct_literal_diagnostic.sh"
)
STANDALONE_COMPILER_TARGETS=(
    "$ROOT_DIR/tests/misc_root_tests/seen_cli_surface.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_atomic_text_io.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_extern_runtime_declaration_dedup.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_semantic_foundation.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_comptime_fail_closed.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_parallel_for_codegen_regression.sh"
)
PLATFORM_TARGETS=(
    "$ROOT_DIR/tests/misc_root_tests/seen_vk_readback_shim.sh"
    "$ROOT_DIR/tests/misc_root_tests/seen_macos_package_client_packaging.sh"
)
LEGACY_DISABLED=(
    "$ROOT_DIR/tests/misc_root_tests/test_incremental.sh"
    "$ROOT_DIR/tests/misc_root_tests/test_pgo.sh"
    "$ROOT_DIR/tests/misc_root_tests/test_sanitizers.sh"
    "$ROOT_DIR/tests/misc_root_tests/test_simd_flags.sh"
    "$ROOT_DIR/tests/misc_root_tests/test_simd_integration.sh"
)
BLOCKED_DISABLED=(
    "$ROOT_DIR/tests/misc_root_tests/seen_fix_regressions.sh"
)

fail() {
    echo "FAIL: capped regression wiring: $*" >&2
    exit 1
}

if [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" = "1" ]; then
    # shellcheck source=scripts/artifact_root.sh
    source "$ARTIFACT_HELPER" || fail "could not load artifact-root helper"
    # shellcheck source=scripts/serial_auxiliary_env.sh
    source "$SERIAL_AUXILIARY" || fail "could not load serial-auxiliary helper"
    seen_artifact_root_init "$ROOT_DIR" || fail "artifact-root validation"
    seen_serial_auxiliary_verify "$ROOT_DIR" "$SEEN_ARTIFACT_ROOT" ||
        fail "hard-scope auxiliary thread settings were not verified"
fi

command -v rg >/dev/null 2>&1 || fail "rg is required"
bash -n "$ENTRY" "$RUNNER" "$STAGE1_ACCEPTANCE" "${TARGETS[@]}" \
    "${STANDALONE_COMPILER_TARGETS[@]}" "${PLATFORM_TARGETS[@]}" \
    "${LEGACY_DISABLED[@]}" "${BLOCKED_DISABLED[@]}" \
    "${CAPABILITY_CALLERS[@]}" ||
    fail "shell syntax"

if rg -n -U -P \
    'bash "\$BUILDER_CAPABILITY(?:_SCRIPT)?"(?s:.{0,256}?)\|\| true' \
    "${CAPABILITY_CALLERS[@]}"; then

    fail "builder capability caller discards a nonzero classifier status"
fi

for required in \
    run_with_project_artifacts.sh run_in_hard_memory_scope.sh \
    verify_fork_serializer.sh rebuild_builder_applicability.sh \
    rebuild_builder_capability.sh prepare_bounded_toolchain.sh \
    'stat -c '\''%d:%i'\'' /tmp' '--verify-only' \
    '[ "$SEEN_MAIN_VMEM_KB" -le "$SEEN_MEMORY_GUARD_RSS_KB" ]' \
    'MAX_OPT_KB=2097152' 'MAX_TASKS=24'; do

    grep -Fq -- "$required" "$ENTRY" || fail "entry helper omitted $required"
done
namespace_line=$(grep -n -m1 '^validate_artifact_namespace$' "$ENTRY" | cut -d: -f1)
repo_cd_line=$(grep -n -m1 '^cd -- "\$REPO_ROOT"' "$ENTRY" | cut -d: -f1)
scope_line=$(grep -n -m1 '^[[:space:]]*verify_hard_scope$' "$ENTRY" | cut -d: -f1)
case "$namespace_line:$repo_cd_line:$scope_line" in
    *[!0-9:]*) fail "could not prove repository-entry ordering" ;;
esac
[ "$namespace_line" -lt "$repo_cd_line" ] && [ "$repo_cd_line" -lt "$scope_line" ] ||
    fail "repository root must be entered after namespace proof and before hard scope"
grep -Fq 'LD_PRELOAD="$SERIALIZER"' "$RUNNER" ||
    fail "runner omitted the attested serializer"
grep -Fq 'SEEN_FORK_SERIALIZER_TARGET="$COMPILER"' "$RUNNER" ||
    fail "runner does not bind the serializer to the exact compiler target"
grep -Fq 'env -u SEEN_FORK_SERIALIZER_ROOT_PID' "$RUNNER" ||
    fail "runner does not clear inherited serializer root state"
grep -Fq 'SEEN_JOBS=1' "$RUNNER" || fail "runner omitted serial compiler jobs"
grep -Fq 'SEEN_OPT_JOBS=1' "$RUNNER" || fail "runner omitted serial optimizer jobs"
grep -Fq 'SEEN_CAPPED_REGRESSION_TOOLCHAIN' "$ENTRY" ||
    fail "entry helper does not bind the prepared toolchain"
grep -Fq -- '--verify-platform-active' "$ENTRY" ||
    fail "entry helper omitted platform-scope verification"
grep -Fq 'SEEN_CAPPED_PLATFORM_REGRESSION_TOOLCHAIN' "$ENTRY" ||
    fail "entry helper does not bind the platform toolchain"
grep -Fq 'no equivalent non-Linux aggregate hard scope is available' "$ENTRY" ||
    fail "platform route does not fail closed without a Linux hard scope"
platform_os_line=$(grep -n -m1 'no equivalent non-Linux aggregate hard scope is available' \
    "$ENTRY" | cut -d: -f1)
artifact_entry_line=$(grep -n -m1 '^if { \[ "\$MODE" = "run"' "$ENTRY" | cut -d: -f1)
case "$platform_os_line:$artifact_entry_line" in
    *[!0-9:]*) fail "could not prove non-Linux platform fail-closed ordering" ;;
esac
[ "$platform_os_line" -lt "$artifact_entry_line" ] ||
    fail "platform route can create artifacts before rejecting non-Linux"
grep -Fq '"$SEEN_BOUNDED_TOOLCHAIN_DIR"|"$SEEN_BOUNDED_TOOLCHAIN_DIR":*' \
    "$RUNNER" || fail "runner does not require bounded toolchain PATH precedence"
grep -Fq -- '--usage-only-invalid-worker' "$RUNNER" ||
    fail "runner omitted the bounded invalid-worker CLI probe"
grep -Fq 'invalid-worker usage probe source must not exist' "$RUNNER" ||
    fail "invalid-worker CLI probe could reach a real source"
grep -Fq 'skip_worker_insert=$has_help' "$RUNNER" ||
    fail "compile help probes are polluted by inserted worker options"
grep -Fq 'DIRECT_COMPILER="$COMPILER_WRAPPER"' "$STAGE1_ACCEPTANCE" ||
    fail "Stage-1 acceptance does not isolate its direct serializer shim"
grep -Fq 'COMPILER="$REAL_COMPILER"' "$STAGE1_ACCEPTANCE" ||
    fail "Stage-1 acceptance does not export the real compiler identity"
grep -Fq 'SEEN_BIN="$REAL_COMPILER"' "$STAGE1_ACCEPTANCE" ||
    fail "Stage-1 acceptance does not export the real SEEN_BIN identity"
if grep -Eq '^[[:space:]]*COMPILER="\$COMPILER_WRAPPER"' \
    "$STAGE1_ACCEPTANCE" ||
    grep -Eq '^[[:space:]]*SEEN_BIN="\$COMPILER_WRAPPER"' \
        "$STAGE1_ACCEPTANCE"; then

    fail "Stage-1 acceptance exposes its shell shim to nested containment"
fi
if grep -Fq 'PATH="$COMPILER_WRAPPER_DIR:$PATH"' "$STAGE1_ACCEPTANCE"; then
    fail "Stage-1 acceptance places its shim ahead of the bounded toolchain"
fi
grep -Fq '"$DIRECT_COMPILER" compile' "$STAGE1_ACCEPTANCE" ||
    fail "Stage-1 acceptance-owned compiles bypass the direct serializer shim"
grep -Fq 'local compile_log="$ACCEPTANCE_ROOT/$label.compile.log"' \
    "$STAGE1_ACCEPTANCE" ||
    fail "Stage-1 acceptance does not keep a per-fixture compiler log"
grep -Fq '"${COMPILER_WORKER_FLAGS[@]}" >"$compile_log" 2>&1' \
    "$STAGE1_ACCEPTANCE" ||
    fail "Stage-1 acceptance does not capture both compiler output streams"
grep -Fq 'compile_status=$?' "$STAGE1_ACCEPTANCE" ||
    fail "Stage-1 acceptance does not capture the exact compiler status"
grep -Fq 'return "$compile_status"' "$STAGE1_ACCEPTANCE" ||
    fail "Stage-1 acceptance collapses the compiler failure status"
grep -Fq 'GOMAXPROCS=1 GOFLAGS="-p=1 -modcacherw"' \
    "$STAGE1_ACCEPTANCE" ||
    fail "Stage-1 Go tests are not serial or their cache is not cleanup-safe"
grep -Fq 'tail -c "$FIXTURE_LOG_TAIL_BYTES" -- "$compile_log"' \
    "$STAGE1_ACCEPTANCE" ||
    fail "Stage-1 acceptance does not print a bounded compiler log tail"
grep -Fq 'WINE_OVERRIDES="explorer.exe,services.exe,winemenubuilder.exe=d"' \
    "$ATOMIC_TEXT_IO" ||
    fail "atomic I/O Wine fixture can start an unbounded service topology"
grep -Fq 'WINE_CPU_TOPOLOGY="1:1"' "$ATOMIC_TEXT_IO" ||
    fail "atomic I/O Wine fixture does not bound its virtual CPU topology"
grep -Fq 'taskset -c "$wine_cpu"' "$ATOMIC_TEXT_IO" ||
    fail "atomic I/O Wine fixture does not bind its host CPU topology"
grep -Fq '"$SEEN_WINE_PREFIX_TEMPLATE/." "$WINE_PREFIX/"' "$ATOMIC_TEXT_IO" ||
    fail "atomic I/O Wine fixture does not clone the bounded prefix template"
grep -Fq 'env WINEPREFIX="$WINE_PREFIX" wineserver -w' "$ATOMIC_TEXT_IO" ||
    fail "atomic I/O Wine fixture does not wait for server cleanup"
grep -Fq 'wine_runtime_fits_task_scope' "$ATOMIC_TEXT_IO" ||
    fail "atomic I/O Wine fixture ignores aggregate task headroom"
grep -Fq '[ "$task_limit" -gt 24 ]' "$ATOMIC_TEXT_IO" ||
    fail "atomic I/O Wine fixture can saturate the verified 24-task scope"
grep -Fq 'Windows binary compiled' "$ATOMIC_TEXT_IO" ||
    fail "atomic I/O bounded fallback does not retain its cross-compile gate"
grep -Fq 'elif [ "$status" -ne 0 ]; then' "$STAGE1_ACCEPTANCE" ||
    fail "Stage-1 acceptance cleanup does not retain failed fixture artifacts"
grep -Fq 'Stage-1 acceptance failure artifacts retained:' \
    "$STAGE1_ACCEPTANCE" ||
    fail "Stage-1 acceptance does not report retained failure artifacts"
grep -Fq 'bash "$REPO_ROOT/installer/test/test-runner.sh" --quick' \
    "$STAGE1_ACCEPTANCE" ||
    fail "Stage-1 acceptance executes a non-executable installer test directly"
grep -Fq 'bash "$REPO_ROOT/installer/test/integration-test.sh"' \
    "$STAGE1_ACCEPTANCE" ||
    fail "Stage-1 acceptance executes a non-executable integration test directly"
grep -Fq 'if [ "$status" -eq 0 ] && [ "$cleanup_status" -ne 0 ]; then' \
    "$STAGE1_ACCEPTANCE" ||
    fail "Stage-1 acceptance cleanup can collapse a resource failure status"
cleanup_trap_line=$(grep -n -m1 '^[[:space:]]*trap - EXIT$' \
    "$STAGE1_ACCEPTANCE" | cut -d: -f1)
cleanup_exit_line=$(grep -n -m1 '^[[:space:]]*exit "\$status"$' \
    "$STAGE1_ACCEPTANCE" | cut -d: -f1)
case "$cleanup_trap_line" in
    ''|*[!0-9]*) fail "Stage-1 acceptance cleanup does not disable its EXIT trap" ;;
esac
case "$cleanup_exit_line" in
    ''|*[!0-9]*) fail "Stage-1 acceptance cleanup does not exit with its resolved status" ;;
esac
[ "$cleanup_trap_line" -lt "$cleanup_exit_line" ] ||
    fail "Stage-1 acceptance cleanup can recursively invoke its EXIT trap"

cleanup_function=$(sed -n '/^cleanup() {/,/^}/p' "$STAGE1_ACCEPTANCE")
[ -n "$cleanup_function" ] || fail "could not extract Stage-1 cleanup function"
probe_cleanup_status() {
    local requested_status=$1
    local expected_status=$2
    local actual_status
    local probe_output

    if probe_output=$(
        {
            printf '%s\n' "$cleanup_function"
            printf '%s\n' \
                'set -u' \
                'requested_status=$1' \
                'SEEN_ARTIFACT_ROOT="$2/.seen/agent-tools/cleanup-status-probe"' \
                'TIER=quick' \
                'ACCEPTANCE_ROOT="$SEEN_ARTIFACT_ROOT/stage1-quick.missing"' \
                'rm() { return 1; }' \
                'trap cleanup EXIT' \
                'exit "$requested_status"'
        } | bash -s -- "$requested_status" "$ROOT_DIR" 2>&1
    ); then
        actual_status=0
    else
        actual_status=$?
    fi
    [ "$actual_status" -eq "$expected_status" ] ||
        fail "cleanup status probe requested $requested_status, expected $expected_status, got $actual_status: $probe_output"
}
probe_cleanup_status 0 1
for resource_status in 124 125 126 137 143; do
    probe_cleanup_status "$resource_status" "$resource_status"
done
grep -Fq '"stage1-acceptance-$TIER" --keep-on-failure --' \
    "$STAGE1_ACCEPTANCE" ||
    fail "standalone Stage-1 acceptance can discard its failed run root"
if rg -n -U -P \
    '"\$DIRECT_COMPILER" compile(?s:.{0,384}?)(?:1?>)[[:space:]]*/dev/null' \
    "$STAGE1_ACCEPTANCE"; then

    fail "Stage-1 acceptance discards fixture compiler diagnostics"
fi
grep -Fq '"$DIRECT_COMPILER" --version' "$STAGE1_ACCEPTANCE" ||
    fail "Stage-1 acceptance version probe bypasses the direct serializer shim"
grep -Fq '"$DIRECT_COMPILER" --help' "$STAGE1_ACCEPTANCE" ||
    fail "Stage-1 acceptance help probe bypasses the direct serializer shim"
if rg -n '(^|[[:space:]])eval([[:space:]]|$)' "$ENTRY" "$RUNNER"; then
    fail "containment helpers must preserve argv without eval"
fi

for target in "${TARGETS[@]}"; do
    grep -Fq 'run_capped_regression.sh' "$target" ||
        fail "missing capped entry in $(basename "$target")"
    grep -Fq 'bash "$ATTESTED_SEEN"' "$target" ||
        fail "missing attested compiler runner in $(basename "$target")"
    grep -Fq 'mktemp -d "$SEEN_ARTIFACT_ROOT/' "$target" ||
        fail "fixture work root is not project-local in $(basename "$target")"

    verify_line=$(grep -n -m1 -- '--verify-active' "$target" | cut -d: -f1)
    work_line=$(grep -n -m1 'TMP_DIR=.*mktemp' "$target" | cut -d: -f1)
    case "$verify_line" in
        ''|*[!0-9]*) fail "could not prove entry ordering in $(basename "$target")" ;;
    esac
    case "$work_line" in
        ''|*[!0-9]*) fail "could not prove output ordering in $(basename "$target")" ;;
    esac
    [ "$verify_line" -lt "$work_line" ] ||
        fail "fixture output precedes scope read-back in $(basename "$target")"

    if rg -n 'mktemp[^[:cntrl:]]*/tmp|TMP_DIR=[^[:cntrl:]]*/tmp|rm -rf[[:space:]]+[^[:cntrl:]]*/tmp' \
        "$target"; then

        fail "host temporary path operation remains in $(basename "$target")"
    fi
    if rg -n 'SEEN_TEST_NO_ULIMIT|14680064|10485760|8388608' "$target"; then
        fail "legacy permissive memory cap remains in $(basename "$target")"
    fi
    if rg -n -- '--no-fork|--jobs(?:=|[[:space:]])|--opt-jobs(?:=|[[:space:]])' \
        "$target"; then

        fail "fixture bypasses attested worker-schema selection in $(basename "$target")"
    fi

    while IFS=: read -r _ line text; do
        [ -n "$line" ] || continue
        case "$text" in
            *ATTESTED_SEEN*) ;;
            *) fail "direct compiler route at $(basename "$target"):$line" ;;
        esac
    done < <(rg -n --with-filename \
        '\$\{?(COMPILER|SEEN_BIN|CWD_COMPILER)\}?"?[[:space:]]+(compile|run|pkg|check|import-c)' \
        "$target" || true)
done

for target in "${STANDALONE_COMPILER_TARGETS[@]}"; do
    grep -Fq 'run_capped_regression.sh' "$target" ||
        fail "missing capped entry in $(basename "$target")"
    grep -Fq 'bash "$ATTESTED_SEEN" "$COMPILER"' "$target" ||
        fail "missing attested compiler runner in $(basename "$target")"
    grep -Fq '="$SEEN_ARTIFACT_ROOT"' "$target" ||
        fail "fixture does not bind its work root to the private artifact root: $(basename "$target")"

    verify_line=$(grep -n -m1 -- '--verify-active' "$target" | cut -d: -f1)
    work_line=$(grep -En -m1 '(TMP_DIR|TEST_ROOT|WORK_DIR)="?\$\(mktemp' \
        "$target" | cut -d: -f1)
    case "$verify_line:$work_line" in
        *[!0-9:]*) fail "could not prove standalone entry ordering in $(basename "$target")" ;;
    esac
    [ "$verify_line" -lt "$work_line" ] ||
        fail "standalone fixture output precedes scope proof in $(basename "$target")"

    if rg -n 'mktemp[^[:cntrl:]]*/tmp|=(?:/tmp|"/tmp)|rm -rf[[:space:]]+[^[:cntrl:]]*/tmp' \
        "$target"; then

        fail "host temporary path operation remains in $(basename "$target")"
    fi
    if [ "$(basename "$target")" != "seen_cli_surface.sh" ] &&
        rg -n -- '--no-fork|--jobs(?:=|[[:space:]])|--opt-jobs(?:=|[[:space:]])' \
            "$target"; then

        fail "standalone fixture bypasses worker-schema selection in $(basename "$target")"
    fi
    while IFS=: read -r _ line text; do
        [ -n "$line" ] || continue
        case "$text" in
            *ATTESTED_SEEN*|*run_compiler*|*seen_command*|*invalid_worker_usage*) ;;
            *) fail "direct standalone compiler route at $(basename "$target"):$line" ;;
        esac
    done < <(rg -n --with-filename \
        '\$\{?COMPILER\}?"?[[:space:]]+(compile|run|pkg|check|import-c|translate)' \
        "$target" || true)
done

for target in "${PLATFORM_TARGETS[@]}"; do
    grep -Fq 'run_capped_regression.sh' "$target" ||
        fail "missing capped platform entry in $(basename "$target")"
    grep -Fq -- '--verify-platform-active' "$target" ||
        fail "missing platform scope read-back in $(basename "$target")"
    grep -Fq 'mktemp -d "$SEEN_ARTIFACT_ROOT/' "$target" ||
        fail "platform fixture work root is not private in $(basename "$target")"
    verify_line=$(grep -n -m1 -- '--verify-platform-active' "$target" | cut -d: -f1)
    work_line=$(grep -n -m1 'TMP_DIR=.*mktemp' "$target" | cut -d: -f1)
    case "$verify_line:$work_line" in
        *[!0-9:]*) fail "could not prove platform entry ordering in $(basename "$target")" ;;
    esac
    [ "$verify_line" -lt "$work_line" ] ||
        fail "platform output precedes scope proof in $(basename "$target")"
    if rg -n 'mktemp[^[:cntrl:]]*/tmp|TMP_DIR=[^[:cntrl:]]*/tmp|SEEN_TEST_VMEM_KB|16777216' \
        "$target"; then

        fail "platform fixture retains a host path or permissive cap: $(basename "$target")"
    fi
done
grep -Fq 'run_helper_capped cc' \
    "$ROOT_DIR/tests/misc_root_tests/seen_vk_readback_shim.sh" ||
    fail "Vulkan C compiler is not helper-capped"
grep -Fq 'PATH="$SEEN_BOUNDED_TOOLCHAIN_DIR:$FAKE_BIN:$PATH"' \
    "$ROOT_DIR/tests/misc_root_tests/seen_macos_package_client_packaging.sh" ||
    fail "macOS fixture can displace the bounded toolchain"

for legacy in "${LEGACY_DISABLED[@]}"; do
    stop_line=$(grep -n -m1 'RESOURCE STOP: legacy' "$legacy" | cut -d: -f1)
    exit_line=$(grep -n -m1 '^exit 126$' "$legacy" | cut -d: -f1)
    first_tmp_line=$(grep -n -m1 '/tmp' "$legacy" | cut -d: -f1 || true)
    case "$stop_line:$exit_line" in
        *[!0-9:]*) fail "legacy guard is malformed in $(basename "$legacy")" ;;
    esac
    [ "$stop_line" -lt "$exit_line" ] ||
        fail "legacy guard ordering is invalid in $(basename "$legacy")"
    if [ -n "$first_tmp_line" ]; then
        [ "$exit_line" -lt "$first_tmp_line" ] ||
            fail "legacy fixture can reach a host temporary path: $(basename "$legacy")"
    fi
done

for blocked in "${BLOCKED_DISABLED[@]}"; do
    stop_line=$(grep -n -m1 'RESOURCE STOP: fix regressions require an attested bounded fault-injection toolchain' \
        "$blocked" | cut -d: -f1)
    exit_line=$(grep -n -m1 '^exit 126$' "$blocked" | cut -d: -f1)
    first_tmp_line=$(grep -n -m1 '/tmp' "$blocked" | cut -d: -f1 || true)
    case "$stop_line:$exit_line" in
        *[!0-9:]*) fail "blocked regression guard is malformed in $(basename "$blocked")" ;;
    esac
    [ "$stop_line" -lt "$exit_line" ] ||
        fail "blocked regression guard ordering is invalid in $(basename "$blocked")"
    if [ -n "$first_tmp_line" ]; then
        [ "$exit_line" -lt "$first_tmp_line" ] ||
            fail "blocked regression can reach a host temporary path: $(basename "$blocked")"
    fi
done

echo "PASS: compiler-capable regressions require project-local hard containment"
