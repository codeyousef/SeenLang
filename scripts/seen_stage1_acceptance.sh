#!/usr/bin/env bash
# Focused Stage-1 acceptance against one explicitly selected fresh compiler.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
ARTIFACT_WRAPPER="$SCRIPT_DIR/run_with_project_artifacts.sh"
ARTIFACT_ROOT_SCRIPT="$SCRIPT_DIR/artifact_root.sh"
HARD_SCOPE_WRAPPER="$SCRIPT_DIR/run_in_hard_memory_scope.sh"
BUILDER_CAPABILITY="$SCRIPT_DIR/rebuild_builder_capability.sh"
BUILDER_APPLICABILITY="$SCRIPT_DIR/rebuild_builder_applicability.sh"
SERIALIZER_VERIFY="$SCRIPT_DIR/verify_fork_serializer.sh"
BOUNDED_TOOLCHAIN_PREPARE="$SCRIPT_DIR/prepare_bounded_toolchain.sh"
TIER=""
COMPILER=""
FIXTURE_LOG_TAIL_BYTES=32768

[ -f "$ARTIFACT_ROOT_SCRIPT" ] || {
    echo "ERROR: missing artifact-root helper: $ARTIFACT_ROOT_SCRIPT" >&2
    exit 1
}
# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_ROOT_SCRIPT"

usage() {
    cat <<EOF
Usage: $0 --tier quick|verify --compiler <fresh-compiler>

Runs the focused 0.11 Stage-1 acceptance surface against exactly the compiler
path supplied by the caller. All generated files stay below the repository's
ignored .seen/agent-tools tree.
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --tier)
            [ "$#" -ge 2 ] || {
                echo "ERROR: --tier requires quick or verify" >&2
                exit 2
            }
            TIER=$2
            shift 2
            ;;
        --tier=*)
            TIER=${1#--tier=}
            shift
            ;;
        --compiler)
            [ "$#" -ge 2 ] || {
                echo "ERROR: --compiler requires an executable path" >&2
                exit 2
            }
            COMPILER=$2
            shift 2
            ;;
        --compiler=*)
            COMPILER=${1#--compiler=}
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "ERROR: unknown Stage-1 acceptance option: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

case "$TIER" in
    quick|verify) ;;
    *)
        echo "ERROR: --tier must be quick or verify" >&2
        exit 2
        ;;
esac

[ -n "$COMPILER" ] || {
    echo "ERROR: --compiler is required; PATH compilers are never accepted" >&2
    exit 2
}
case "$COMPILER" in
    /*) ;;
    *) COMPILER="$PWD/$COMPILER" ;;
esac
[ -x "$COMPILER" ] || {
    echo "ERROR: fresh compiler is not executable: $COMPILER" >&2
    exit 1
}
COMPILER="$(cd "$(dirname "$COMPILER")" && pwd -P)/$(basename "$COMPILER")"
REAL_COMPILER="$COMPILER"

is_positive_integer() {
    case "$1" in
        ''|*[!0-9]*) return 1 ;;
        *) [ "$1" -gt 0 ] 2>/dev/null ;;
    esac
}

prepare_stage1_go_cache() {
    local variable_name=$1
    local candidate=$2
    local cache_base="${SEEN_ARTIFACT_ROOT:-}"
    local relative_path
    local canonical_base
    local canonical_candidate

    case "$candidate" in
        "$cache_base"/*) relative_path=${candidate#"$REPO_ROOT"/} ;;
        *)
            echo "ERROR: $variable_name must stay below the validated project artifact root" >&2
            return 1
            ;;
    esac
    seen_artifact_assert_safe_relative_path "$relative_path" || return 1
    seen_artifact_assert_no_symlink_components "$REPO_ROOT" "$relative_path" || return 1
    mkdir -p -- "$candidate" || {
        echo "ERROR: could not create $variable_name directory: $candidate" >&2
        return 1
    }
    seen_artifact_assert_no_symlink_components "$REPO_ROOT" "$relative_path" || return 1
    canonical_base=$(seen_artifact_canonical_dir "$cache_base") || {
        echo "ERROR: could not resolve Stage-1 cache base: $cache_base" >&2
        return 1
    }
    canonical_candidate=$(seen_artifact_canonical_dir "$candidate") || {
        echo "ERROR: could not resolve $variable_name directory: $candidate" >&2
        return 1
    }
    case "$canonical_candidate" in
        "$canonical_base"/*) ;;
        *)
            echo "ERROR: resolved $variable_name escaped the project artifact root" >&2
            return 1
            ;;
    esac
    printf '%s\n' "$canonical_candidate"
}

if ! is_positive_integer "${SEEN_MAIN_VMEM_KB:-}"; then
    echo "ERROR: SEEN_MAIN_VMEM_KB must be an explicit positive cap" >&2
    exit 2
fi
if ! is_positive_integer "${SEEN_OPT_VMEM_KB:-}"; then
    echo "ERROR: SEEN_OPT_VMEM_KB must be an explicit positive cap" >&2
    exit 2
fi
if ! is_positive_integer "${SEEN_MEMORY_LIMIT_BYTES:-}"; then
    echo "ERROR: SEEN_MEMORY_LIMIT_BYTES must be an explicit positive cap" >&2
    exit 2
fi

# Standalone acceptance enters the same project-local namespace used by rebuilds.
# A rebuild that already validated its private /tmp sets the same marker.
if [ "${SEEN_PROJECT_ARTIFACT_WRAPPER:-0}" != "1" ] ||
    [ "${SEEN_PROJECT_ARTIFACT_NAMESPACE_ACTIVE:-0}" != "1" ]; then
    [ -x "$ARTIFACT_WRAPPER" ] || {
        echo "ERROR: missing project artifact wrapper: $ARTIFACT_WRAPPER" >&2
        exit 1
    }
    exec "$ARTIFACT_WRAPPER" "stage1-acceptance-$TIER" --keep-on-failure -- \
        env \
            SEEN_LOW_MEMORY=1 \
            SEEN_MAIN_VMEM_KB="$SEEN_MAIN_VMEM_KB" \
            SEEN_OPT_VMEM_KB="$SEEN_OPT_VMEM_KB" \
            SEEN_MEMORY_LIMIT_BYTES="$SEEN_MEMORY_LIMIT_BYTES" \
            "$0" --tier "$TIER" --compiler "$COMPILER"
fi

seen_artifact_root_init "$REPO_ROOT" || {
    echo "ERROR: Stage-1 acceptance artifact root validation failed" >&2
    exit 1
}
[ -d "$SEEN_ARTIFACT_ROOT" ] && [ ! -L "$SEEN_ARTIFACT_ROOT" ] || {
    echo "ERROR: unsafe Stage-1 acceptance artifact root: $SEEN_ARTIFACT_ROOT" >&2
    exit 1
}
if [ "$(uname -s)" = "Linux" ]; then
    namespace_tmp_identity="$(stat -c '%d:%i' /tmp 2>/dev/null || true)"
    artifact_root_identity="$(stat -c '%d:%i' "$SEEN_ARTIFACT_ROOT" \
        2>/dev/null || true)"
    if [ -z "$namespace_tmp_identity" ] ||
        [ "$namespace_tmp_identity" != "$artifact_root_identity" ]; then

        echo "ERROR: Stage-1 acceptance artifact namespace validation failed" >&2
        exit 1
    fi
fi

if [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != "1" ] &&
    [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" != "1" ]; then
    [ -x "$HARD_SCOPE_WRAPPER" ] || {
        echo "ERROR: missing hard-memory-scope wrapper: $HARD_SCOPE_WRAPPER" >&2
        exit 1
    }
    exec "$HARD_SCOPE_WRAPPER" --label "Stage-1 acceptance $TIER" -- \
        "$0" --tier "$TIER" --compiler "$COMPILER"
fi
SEEN_HARD_MEMORY_SCOPE_ACTIVE=1
export SEEN_HARD_MEMORY_SCOPE_ACTIVE
"$HARD_SCOPE_WRAPPER" --label "Stage-1 acceptance $TIER read-back" \
    --verify-only --

if ! ulimit -S -v "$SEEN_MAIN_VMEM_KB" 2>/dev/null; then
    echo "RESOURCE STOP: could not apply Stage-1 acceptance main cap" >&2
    exit 126
fi
active_main_vmem=$(ulimit -S -v 2>/dev/null || true)
if ! is_positive_integer "$active_main_vmem" ||
    [ "$active_main_vmem" -gt "$SEEN_MAIN_VMEM_KB" ]; then

    echo "RESOURCE STOP: Stage-1 acceptance main cap read-back failed" >&2
    exit 126
fi
export SEEN_LOW_MEMORY=1
export SEEN_DATA_PATH="$REPO_ROOT/languages"
export SEEN_BIN="$COMPILER"
export COMPILER

FORK_SERIALIZER_SO=${SEEN_FORK_SERIALIZER_SO:-}
FORK_SERIALIZER_ATTESTATION=${SEEN_FORK_SERIALIZER_ATTESTATION:-}
if ! bash "$SERIALIZER_VERIFY" "$FORK_SERIALIZER_SO" \
    "$FORK_SERIALIZER_ATTESTATION" "$SEEN_ARTIFACT_ROOT" \
    "${SEEN_MEMORY_GUARD_SCOPE_UNIT:-}" >/dev/null; then

    echo "ERROR: Stage-1 acceptance requires the scope-attested fork serializer produced by safe_rebuild" >&2
    exit 126
fi
if ! SEEN_MEMORY_GUARD_IN_SCOPE=1 bash "$BUILDER_APPLICABILITY" \
    "$REAL_COMPILER" "$FORK_SERIALIZER_SO" >/dev/null; then

    echo "ERROR: Stage-1 acceptance compiler is not serializer-applicable" >&2
    exit 126
fi
BOUNDED_TOOLCHAIN_DIR=$(bash "$BOUNDED_TOOLCHAIN_PREPARE" "$SEEN_ARTIFACT_ROOT") ||
    exit 126
PATH="$BOUNDED_TOOLCHAIN_DIR:$PATH"
export PATH SEEN_BOUNDED_TOOLCHAIN_DIR="$BOUNDED_TOOLCHAIN_DIR"

compiler_capability_status=0
compiler_capability=$(env -u LD_PRELOAD -u SEEN_FORK_SERIALIZER_TARGET \
    -u SEEN_FORK_SERIALIZER_ROOT_PID \
    bash "$BUILDER_CAPABILITY" "$REAL_COMPILER" 2>/dev/null) ||
    compiler_capability_status=$?
if [ "$compiler_capability_status" -ne 0 ]; then
    echo "ERROR: acceptance compiler schema probe failed with status $compiler_capability_status" >&2
    exit 126
fi
case "$compiler_capability" in
    advertised-jobs) COMPILER_WORKER_FLAGS=(--jobs 1 --opt-jobs 1) ;;
    advertised-no-fork) COMPILER_WORKER_FLAGS=(--no-fork) ;;
    serializer-required) COMPILER_WORKER_FLAGS=() ;;
    *) echo "ERROR: acceptance compiler schema probe failed" >&2; exit 126 ;;
esac

ACCEPTANCE_ROOT="$(mktemp -d "$SEEN_ARTIFACT_ROOT/stage1-$TIER.XXXXXX")"
cleanup() {
    local status=$?
    local cleanup_status=0

    case "$ACCEPTANCE_ROOT" in
        "$SEEN_ARTIFACT_ROOT"/stage1-"$TIER".*)
            if [ ! -d "$ACCEPTANCE_ROOT" ] || [ -L "$ACCEPTANCE_ROOT" ] ||
                [ "$(dirname -- "$ACCEPTANCE_ROOT")" != "$SEEN_ARTIFACT_ROOT" ]; then

                echo "ERROR: refusing to clean unsafe acceptance path: $ACCEPTANCE_ROOT" >&2
                cleanup_status=1
            elif [ "$status" -ne 0 ]; then
                printf 'Stage-1 acceptance failure artifacts retained: %s\n' \
                    "$ACCEPTANCE_ROOT" >&2
            elif ! find -P "$ACCEPTANCE_ROOT" -type d \
                -exec chmod u+rwx -- {} +; then

                echo "ERROR: could not make acceptance directories removable: $ACCEPTANCE_ROOT" >&2
                cleanup_status=1
            elif ! rm -rf -- "$ACCEPTANCE_ROOT"; then
                echo "ERROR: could not clean acceptance path: $ACCEPTANCE_ROOT" >&2
                cleanup_status=1
            fi
            ;;
        *)
            echo "ERROR: refusing to clean unexpected acceptance path: $ACCEPTANCE_ROOT" >&2
            cleanup_status=1
            ;;
    esac

    # Never replace a fixture or containment failure (including
    # 124/125/126/137/143) with a cleanup status.
    if [ "$status" -eq 0 ] && [ "$cleanup_status" -ne 0 ]; then
        status=$cleanup_status
    fi
    # An EXIT trap's return status does not replace an original successful
    # exit. Disable this trap before exiting explicitly so cleanup failure is
    # observable without recursively invoking cleanup.
    trap - EXIT
    exit "$status"
}
trap cleanup EXIT
COMPILER_WRAPPER_DIR="$ACCEPTANCE_ROOT/compiler-bin"
mkdir -p -- "$COMPILER_WRAPPER_DIR"
COMPILER_WRAPPER="$COMPILER_WRAPPER_DIR/seen"
printf -v quoted_real_compiler '%q' "$REAL_COMPILER"
printf -v quoted_serializer '%q' "$FORK_SERIALIZER_SO"
{
    printf '%s\n' '#!/usr/bin/env bash'
    printf '%s\n' 'set -euo pipefail'
    printf 'exec env -u SEEN_FORK_SERIALIZER_ROOT_PID LD_PRELOAD=%s SEEN_FORK_SERIALIZER_TARGET=%s %s "$@"\n' \
        "$quoted_serializer" "$quoted_real_compiler" "$quoted_real_compiler"
} > "$COMPILER_WRAPPER"
chmod 700 "$COMPILER_WRAPPER"
DIRECT_COMPILER="$COMPILER_WRAPPER"
# Acceptance-owned invocations use the serializer shim directly. Export the
# real executable identity so nested, self-contained regressions can attest it
# with readelf instead of accidentally trying to attest this shell script.
COMPILER="$REAL_COMPILER"
SEEN_BIN="$REAL_COMPILER"
export COMPILER SEEN_BIN
export SEEN_TEST_ARTIFACT_ROOT="$ACCEPTANCE_ROOT/tests"
mkdir -p -- "$SEEN_TEST_ARTIFACT_ROOT"

EXPECTED_VERSION="$(awk -F'"' '/^version = / { print $2; exit }' "$REPO_ROOT/Seen.toml")"
[ -n "$EXPECTED_VERSION" ] || {
    echo "ERROR: could not derive the checkout version" >&2
    exit 1
}

print_fixture_compile_log() {
    local label=$1
    local compile_log=$2

    # Reporting is best-effort so it can never replace the compiler status.
    if [ ! -f "$compile_log" ] || [ -L "$compile_log" ]; then
        echo "ERROR: $label compiler log is missing or unsafe: $compile_log" >&2
        return 0
    fi
    if [ ! -s "$compile_log" ]; then
        echo "Compiler log is empty: $compile_log" >&2
        return 0
    fi

    printf '%s\n' \
        "--- $label compiler log tail (at most $FIXTURE_LOG_TAIL_BYTES bytes) ---" >&2
    if ! tail -c "$FIXTURE_LOG_TAIL_BYTES" -- "$compile_log" >&2; then
        echo "ERROR: could not read compiler log: $compile_log" >&2
    fi
    printf '\n%s\n' "--- end $label compiler log tail ---" >&2
}

compile_fixture() {
    local label=$1
    local fixture=$2
    local output="$ACCEPTANCE_ROOT/$label"
    local compile_log="$ACCEPTANCE_ROOT/$label.compile.log"
    local compile_status

    case "$label" in
        ''|.|..|*[!A-Za-z0-9._-]*)
            echo "ERROR: unsafe Stage-1 fixture label: $label" >&2
            return 2
            ;;
    esac
    case "$output:$compile_log" in
        "$ACCEPTANCE_ROOT"/*:"$ACCEPTANCE_ROOT"/*.compile.log) ;;
        *)
            echo "ERROR: Stage-1 fixture artifacts escaped the acceptance root" >&2
            return 1
            ;;
    esac
    if [ "$(dirname -- "$output")" != "$ACCEPTANCE_ROOT" ] ||
        [ "$(dirname -- "$compile_log")" != "$ACCEPTANCE_ROOT" ] ||
        [ ! -d "$ACCEPTANCE_ROOT" ] || [ -L "$ACCEPTANCE_ROOT" ]; then

        echo "ERROR: unsafe Stage-1 fixture artifact path for $label" >&2
        return 1
    fi
    if [ -e "$output" ] || [ -L "$output" ] ||
        [ -e "$compile_log" ] || [ -L "$compile_log" ]; then

        echo "ERROR: refusing to overwrite Stage-1 fixture artifacts for $label" >&2
        return 1
    fi

    echo "Stage-1 acceptance: $label"
    if "$DIRECT_COMPILER" compile "$fixture" "$output" --fast --no-cache \
        "${COMPILER_WORKER_FLAGS[@]}" >"$compile_log" 2>&1; then

        compile_status=0
    else
        compile_status=$?
    fi
    if [ "$compile_status" -ne 0 ]; then
        printf 'ERROR: Stage-1 fixture %s compile failed with status %s; log: %s\n' \
            "$label" "$compile_status" "$compile_log" >&2
        print_fixture_compile_log "$label" "$compile_log"
        return "$compile_status"
    fi
    if [ ! -x "$output" ] || [ -L "$output" ]; then
        echo "ERROR: $label did not produce a safe executable; log: $compile_log" >&2
        print_fixture_compile_log "$label" "$compile_log"
        return 1
    fi
}

run_fixture() {
    local label=$1
    local fixture=$2

    compile_fixture "$label" "$fixture"
    local output="$ACCEPTANCE_ROOT/$label"
    "$output"
}

stage_external_package_fixture() {
    local staged_fixtures="$ACCEPTANCE_ROOT/fixtures"
    local staged_package="$staged_fixtures/external_package"
    local staged_consumer="$staged_fixtures/pkg-layout-001/external-consumer"
    local relative

    mkdir -p -- "$staged_package/src" "$staged_package/examples" \
        "$staged_package/tests" "$staged_consumer/src" || return 1
    for relative in Seen.toml Seen.lock README.md LICENSE src/mod.seen \
        examples/consumer.seen tests/layout_contract.seen; do

        cp -- "$REPO_ROOT/tests/fixtures/external_package/$relative" \
            "$staged_package/$relative" || return 1
    done
    for relative in Seen.toml Seen.lock src/main.seen; do
        cp -- \
            "$REPO_ROOT/tests/fixtures/pkg-layout-001/external-consumer/$relative" \
            "$staged_consumer/$relative" || return 1
    done
    printf '%s\n' "$staged_consumer/src/main.seen"
}

compiler_version="$("$DIRECT_COMPILER" --version)"
IFS= read -r compiler_version_line <<< "$compiler_version"
if [ "$compiler_version_line" != "Seen $EXPECTED_VERSION" ]; then
    echo "ERROR: fresh compiler version does not match checkout $EXPECTED_VERSION" >&2
    printf '%s\n' "$compiler_version" >&2
    exit 1
fi
"$DIRECT_COMPILER" --help >/dev/null

SEEN_EXPECTED_VERSION="$EXPECTED_VERSION" \
    "$REPO_ROOT/tests/misc_root_tests/seen_cli_surface.sh"
run_fixture type-ref "$REPO_ROOT/compiler_seen/tests/type_ref.seen"
run_fixture compatibility-manifest \
    "$REPO_ROOT/compiler_seen/tests/compatibility_manifest.seen"
run_fixture package-layout \
    "$REPO_ROOT/compiler_seen/tests/package_layout.seen"
run_fixture import-graph \
    "$REPO_ROOT/compiler_seen/tests/release/import_graph.seen"
run_fixture import-graph-example \
    "$REPO_ROOT/compiler_seen/examples/import_graph_resolution.seen"
run_fixture global-initialization \
    "$REPO_ROOT/compiler_seen/tests/release/global_initialization.seen"
run_fixture global-initialization-example \
    "$REPO_ROOT/compiler_seen/examples/global_initialization_plan.seen"
run_fixture global-initialization-runtime \
    "$REPO_ROOT/tests/fixtures/core-003b/happy/runtime/app.seen"
run_fixture production-ir-policy \
    "$REPO_ROOT/compiler_seen/tests/release/production_ir_policy.seen"
run_fixture production-ir-policy-example \
    "$REPO_ROOT/compiler_seen/examples/production_ir_policy.seen"
run_fixture production-source-policy \
    "$REPO_ROOT/compiler_seen/tests/release/production_source_policy.seen"
run_fixture production-source-policy-example \
    "$REPO_ROOT/compiler_seen/examples/production_source_policy.seen"
run_fixture machine-diagnostic \
    "$REPO_ROOT/compiler_seen/tests/release/machine_diagnostic.seen"
run_fixture machine-diagnostic-example \
    "$REPO_ROOT/compiler_seen/examples/machine_diagnostic.seen"
run_fixture build-instrumentation \
    "$REPO_ROOT/compiler_seen/tests/release/build_instrumentation.seen"
run_fixture build-instrumentation-example \
    "$REPO_ROOT/compiler_seen/examples/build_instrumentation.seen"
run_fixture release-optimization \
    "$REPO_ROOT/compiler_seen/tests/release/release_optimization.seen"
run_fixture release-optimization-example \
    "$REPO_ROOT/compiler_seen/examples/release_optimization.seen"
run_fixture bootstrap-reproducibility \
    "$REPO_ROOT/compiler_seen/tests/reproducibility/core_004a_two_builder.seen"
run_fixture bootstrap-reproducibility-example \
    "$REPO_ROOT/compiler_seen/examples/bootstrap_reproducibility.seen"
run_fixture release-artifact-pins \
    "$REPO_ROOT/compiler_seen/tests/reproducibility/core_004b_artifact_pins.seen"
run_fixture release-artifact-pins-example \
    "$REPO_ROOT/compiler_seen/examples/release_artifact_pins.seen"
run_fixture gate0-certification \
    "$REPO_ROOT/compiler_seen/tests/release/p0_gate0_001_certification.seen"
run_fixture gate0-certification-example \
    "$REPO_ROOT/compiler_seen/examples/gate0_certification.seen"
run_fixture test-discovery \
    "$REPO_ROOT/compiler_seen/tests/test_discovery.seen"
run_fixture test-discovery-example \
    "$REPO_ROOT/compiler_seen/examples/test_discovery.seen"
run_fixture test-runner "$REPO_ROOT/compiler_seen/tests/test_001b_runner.seen"
run_fixture test-runner-example \
    "$REPO_ROOT/compiler_seen/examples/test_runner.seen"
run_fixture test-instrumentation \
    "$REPO_ROOT/compiler_seen/tests/test_002a_instrumentation.seen"
run_fixture test-fuzz-corpus \
    "$REPO_ROOT/compiler_seen/tests/test_002b_fuzz_corpus.seen"
run_fixture test-fuzz-corpus-example \
    "$REPO_ROOT/compiler_seen/examples/test_fuzz_corpus.seen"
run_fixture test-benchmark-evidence \
    "$REPO_ROOT/compiler_seen/tests/test_002c_benchmark_evidence.seen"
run_fixture test-benchmark-evidence-example \
    "$REPO_ROOT/compiler_seen/examples/test_benchmark_evidence.seen"
run_fixture test-leak-soak \
    "$REPO_ROOT/compiler_seen/tests/test_002d_leak_soak.seen"
run_fixture test-leak-soak-example \
    "$REPO_ROOT/compiler_seen/examples/test_leak_soak.seen"
run_fixture test-assertions \
    "$REPO_ROOT/compiler_seen/tests/test_001c_assertions.seen"
run_fixture test-assertions-example \
    "$REPO_ROOT/compiler_seen/examples/test_assertions.seen"
run_fixture test-fixtures \
    "$REPO_ROOT/compiler_seen/tests/test_001d_fixtures.seen"
run_fixture test-fixtures-example \
    "$REPO_ROOT/compiler_seen/examples/test_fixture.seen"
run_fixture test-reporters \
    "$REPO_ROOT/compiler_seen/tests/test_001e_reporters.seen"
run_fixture test-reporters-example \
    "$REPO_ROOT/compiler_seen/examples/test_reporters.seen"
run_fixture test-migration \
    "$REPO_ROOT/compiler_seen/tests/test_001f_migration.seen"
run_fixture test-migration-example \
    "$REPO_ROOT/compiler_seen/examples/test_migration.seen"
run_fixture structured-error \
    "$REPO_ROOT/seen_std/tests/error/err_001a_error_contract.seen"
run_fixture structured-error-example \
    "$REPO_ROOT/seen_std/examples/error_contract.seen"
run_fixture typed-errors \
    "$REPO_ROOT/seen_std/tests/error/err_001b_typed_errors.seen"
run_fixture typed-errors-example \
    "$REPO_ROOT/seen_std/examples/typed_errors.seen"
run_fixture error-api-migration \
    "$REPO_ROOT/seen_std/tests/error/err_001c_error_api_migration.seen"
run_fixture error-api-migration-example \
    "$REPO_ROOT/seen_std/examples/error_api_migration.seen"
run_fixture error-policy \
    "$REPO_ROOT/seen_std/tests/error/err_001d_error_policy.seen"
run_fixture error-policy-example \
    "$REPO_ROOT/seen_std/examples/error_policy.seen"
run_fixture owned-resource \
    "$REPO_ROOT/seen_std/tests/error/p0_own_001_owned_resource.seen"
run_fixture owned-resource-example \
    "$REPO_ROOT/seen_std/examples/owned_resource.seen"
run_fixture secret-markers \
    "$REPO_ROOT/seen_std/tests/error/p0_secret_001_secret_markers.seen"
run_fixture secret-markers-example \
    "$REPO_ROOT/seen_std/examples/secret_values.seen"

gate0_test_log="$ACCEPTANCE_ROOT/gate0-test-cli.log"
if ! "$DIRECT_COMPILER" test "$REPO_ROOT" --filter P0-GATE0-001 \
    --profile ci --jobs 1 --timeout 10m \
    --report json:.seen_cache/test/P0-GATE0-001.json \
    --report junit:.seen_cache/test/P0-GATE0-001.xml \
    >"$gate0_test_log" 2>&1; then

    echo "ERROR: P0-GATE0-001 canonical Seen test failed: $gate0_test_log" >&2
    cat "$gate0_test_log" >&2
    exit 1
fi
grep -Fq 'test result: 1 passed; 0 failed' "$gate0_test_log" || {
    echo "ERROR: P0-GATE0-001 canonical Seen test summary is missing" >&2
    exit 1
}
python3 "$REPO_ROOT/scripts/check_test_reporters.py" --format json \
    --validate "$REPO_ROOT/.seen_cache/test/P0-GATE0-001.json" >/dev/null
python3 "$REPO_ROOT/scripts/check_test_reporters.py" --format junit \
    --validate "$REPO_ROOT/.seen_cache/test/P0-GATE0-001.xml" >/dev/null

test_runner_log="$ACCEPTANCE_ROOT/test-runner-cli.log"
if ! "$DIRECT_COMPILER" test "$REPO_ROOT" --filter TEST-001B \
    --profile ci --jobs 1 --timeout 10m \
    --report json:.seen_cache/test/TEST-001B.json \
    --report junit:.seen_cache/test/TEST-001B.xml >"$test_runner_log" 2>&1; then
    echo "ERROR: canonical seen test acceptance failed: $test_runner_log" >&2
    cat "$test_runner_log" >&2
    exit 1
fi
grep -Fq 'test result: 1 passed; 0 failed' "$test_runner_log" || {
    echo "ERROR: canonical seen test summary is missing" >&2
    exit 1
}
test_migration_log="$ACCEPTANCE_ROOT/test-migration-cli.log"
if ! "$DIRECT_COMPILER" test "$REPO_ROOT" --filter TEST-001F \
    --profile ci --jobs 1 --timeout 10m \
    --report json:.seen_cache/test/TEST-001F.json \
    --report junit:.seen_cache/test/TEST-001F.xml \
    >"$test_migration_log" 2>&1; then

    echo "ERROR: TEST-001F canonical seen test acceptance failed: $test_migration_log" >&2
    cat "$test_migration_log" >&2
    exit 1
fi
grep -Fq 'test result: 1 passed; 0 failed' "$test_migration_log" || {
    echo "ERROR: TEST-001F canonical seen test summary is missing" >&2
    exit 1
}
python3 "$REPO_ROOT/scripts/check_test_reporters.py" --format json \
    --validate "$REPO_ROOT/.seen_cache/test/TEST-001F.json" >/dev/null
python3 "$REPO_ROOT/scripts/check_test_reporters.py" --format junit \
    --validate "$REPO_ROOT/.seen_cache/test/TEST-001F.xml" >/dev/null

error_contract_log="$ACCEPTANCE_ROOT/error-contract-cli.log"
if ! "$DIRECT_COMPILER" test "$REPO_ROOT" --filter ERR-001A \
    --profile ci --jobs 1 --timeout 10m \
    --report json:.seen_cache/test/ERR-001A.json \
    --report junit:.seen_cache/test/ERR-001A.xml \
    >"$error_contract_log" 2>&1; then

    echo "ERROR: ERR-001A canonical seen test acceptance failed: $error_contract_log" >&2
    cat "$error_contract_log" >&2
    exit 1
fi
grep -Fq 'test result: 1 passed; 0 failed' "$error_contract_log" || {
    echo "ERROR: ERR-001A canonical seen test summary is missing" >&2
    exit 1
}
python3 "$REPO_ROOT/scripts/check_test_reporters.py" --format json \
    --validate "$REPO_ROOT/.seen_cache/test/ERR-001A.json" >/dev/null
python3 "$REPO_ROOT/scripts/check_test_reporters.py" --format junit \
    --validate "$REPO_ROOT/.seen_cache/test/ERR-001A.xml" >/dev/null

if [ "${SEEN_ERR_001A_FULL_REPORT:-0}" = "1" ]; then
    error_contract_full_log="$ACCEPTANCE_ROOT/error-contract-full-cli.log"
    error_contract_full_status=0
    "$DIRECT_COMPILER" test "$REPO_ROOT" --profile ci --jobs 1 \
        --timeout 10m --report json:.seen_cache/test/ERR-001A-full.json \
        --report junit:.seen_cache/test/ERR-001A-full.xml \
        >"$error_contract_full_log" 2>&1 || error_contract_full_status=$?
    [ "$error_contract_full_status" -le 1 ] || {
        echo "ERROR: ERR-001A full test infrastructure failed with status $error_contract_full_status" >&2
        cat "$error_contract_full_log" >&2
        exit 1
    }
    python3 "$REPO_ROOT/scripts/check_test_reporters.py" --format json \
        --validate "$REPO_ROOT/.seen_cache/test/ERR-001A-full.json" >/dev/null
    python3 "$REPO_ROOT/scripts/check_test_reporters.py" --format junit \
        --validate "$REPO_ROOT/.seen_cache/test/ERR-001A-full.xml" >/dev/null
    tail -n 1 "$error_contract_full_log"
fi

typed_error_log="$ACCEPTANCE_ROOT/typed-error-cli.log"
if ! "$DIRECT_COMPILER" test "$REPO_ROOT" --filter ERR-001B \
    --profile ci --jobs 1 --timeout 10m \
    --report json:.seen_cache/test/ERR-001B.json \
    --report junit:.seen_cache/test/ERR-001B.xml \
    >"$typed_error_log" 2>&1; then

    echo "ERROR: ERR-001B canonical seen test acceptance failed: $typed_error_log" >&2
    cat "$typed_error_log" >&2
    exit 1
fi
grep -Fq 'test result: 1 passed; 0 failed' "$typed_error_log" || {
    echo "ERROR: ERR-001B canonical seen test summary is missing" >&2
    exit 1
}
python3 "$REPO_ROOT/scripts/check_test_reporters.py" --format json \
    --validate "$REPO_ROOT/.seen_cache/test/ERR-001B.json" >/dev/null
python3 "$REPO_ROOT/scripts/check_test_reporters.py" --format junit \
    --validate "$REPO_ROOT/.seen_cache/test/ERR-001B.xml" >/dev/null

if [ "${SEEN_ERR_001B_FULL_REPORT:-0}" = "1" ]; then
    typed_error_full_log="$ACCEPTANCE_ROOT/typed-error-full-cli.log"
    typed_error_full_status=0
    "$DIRECT_COMPILER" test "$REPO_ROOT" --profile ci --jobs 1 \
        --timeout 10m --report json:.seen_cache/test/ERR-001B-full.json \
        --report junit:.seen_cache/test/ERR-001B-full.xml \
        >"$typed_error_full_log" 2>&1 || typed_error_full_status=$?
    [ "$typed_error_full_status" -le 1 ] || {
        echo "ERROR: ERR-001B full test infrastructure failed with status $typed_error_full_status" >&2
        cat "$typed_error_full_log" >&2
        exit 1
    }
    python3 "$REPO_ROOT/scripts/check_test_reporters.py" --format json \
        --validate "$REPO_ROOT/.seen_cache/test/ERR-001B-full.json" >/dev/null
    python3 "$REPO_ROOT/scripts/check_test_reporters.py" --format junit \
        --validate "$REPO_ROOT/.seen_cache/test/ERR-001B-full.xml" >/dev/null
    tail -n 1 "$typed_error_full_log"
fi

if [ "${SEEN_TEST_001F_FULL_REPORT:-0}" = "1" ]; then
    test_migration_full_log="$ACCEPTANCE_ROOT/test-migration-full-cli.log"
    test_migration_full_status=0
    "$DIRECT_COMPILER" test "$REPO_ROOT" --profile ci --jobs 1 \
        --timeout 10m --report json:.seen_cache/test/TEST-001F-full.json \
        --report junit:.seen_cache/test/TEST-001F-full.xml \
        >"$test_migration_full_log" 2>&1 || test_migration_full_status=$?
    [ "$test_migration_full_status" -le 1 ] || {
        echo "ERROR: TEST-001F full test infrastructure failed with status $test_migration_full_status" >&2
        cat "$test_migration_full_log" >&2
        exit 1
    }
    python3 "$REPO_ROOT/scripts/check_test_reporters.py" --format json \
        --validate "$REPO_ROOT/.seen_cache/test/TEST-001F-full.json" >/dev/null
    python3 "$REPO_ROOT/scripts/check_test_reporters.py" --format junit \
        --validate "$REPO_ROOT/.seen_cache/test/TEST-001F-full.xml" >/dev/null
    tail -n 1 "$test_migration_full_log"
fi
test_runner_invalid_status=0
"$DIRECT_COMPILER" test "$REPO_ROOT" --jobs 2 \
    >"$ACCEPTANCE_ROOT/test-runner-invalid.log" 2>&1 ||
    test_runner_invalid_status=$?
[ "$test_runner_invalid_status" -eq 2 ] || {
    echo "ERROR: canonical seen test invalid-input exit code changed" >&2
    exit 1
}

instrumentation_dir="$ACCEPTANCE_ROOT/core-rel-002"
instrumentation_output="$instrumentation_dir/instrumented"
instrumentation_log="$instrumentation_dir/compile.log"
instrumentation_report="core-rel-002/evidence.json"
mkdir -p -- "$instrumentation_dir"
(
    cd "$ACCEPTANCE_ROOT"
    "$DIRECT_COMPILER" compile \
        "$REPO_ROOT/tests/fixtures/core-rel-002/happy/main.seen" \
        "$instrumentation_output" --fast --no-cache --debug --coverage \
        --sanitize undefined --instrumentation-report \
        "$instrumentation_report" "${COMPILER_WORKER_FLAGS[@]}"
) >"$instrumentation_log" 2>&1 || {
    echo "ERROR: CORE-REL-002 instrumented compile failed: $instrumentation_log" >&2
    print_fixture_compile_log core-rel-002 "$instrumentation_log"
    exit 1
}
"$instrumentation_output"
python3 "$REPO_ROOT/scripts/check_build_instrumentation.py" \
    --evidence "$instrumentation_dir/evidence.json"

release_dir="$ACCEPTANCE_ROOT/core-rel-003"
release_fixture="$REPO_ROOT/tests/fixtures/core-rel-003/happy/main.seen"
release_generate="$release_dir/pgo-generate"
release_use="$release_dir/pgo-use"
release_full="$release_dir/full-lto"
release_raw="$release_dir/default.profraw"
release_profile_rel="core-rel-003/default.profdata"
release_profile="$ACCEPTANCE_ROOT/$release_profile_rel"
release_trace="$release_dir/full-lto.jsonl"
mkdir -p -- "$release_dir"
(
    cd "$ACCEPTANCE_ROOT"
    "$DIRECT_COMPILER" compile "$release_fixture" "$release_generate" \
        --release --lto thin --pgo-generate --no-cache \
        "${COMPILER_WORKER_FLAGS[@]}"
    LLVM_PROFILE_FILE="$release_raw" "$release_generate"
    llvm-profdata merge -sparse "$release_raw" -o "$release_profile"
    "$DIRECT_COMPILER" compile "$release_fixture" "$release_use" \
        --release --lto thin --pgo-use "$release_profile_rel" --no-cache \
        "${COMPILER_WORKER_FLAGS[@]}"
    "$release_use"
    SEEN_BUILD_TRACE="$release_trace" SEEN_TRACE_BUILD="$release_trace" \
        "$DIRECT_COMPILER" compile "$release_fixture" "$release_full" \
        --release --lto full --no-cache "${COMPILER_WORKER_FLAGS[@]}"
) >"$release_dir/acceptance.log" 2>&1 || {
    echo "ERROR: CORE-REL-003 release acceptance failed: $release_dir/acceptance.log" >&2
    print_fixture_compile_log core-rel-003 "$release_dir/acceptance.log"
    exit 1
}
"$release_full"
grep -Fq '"phase":"release lto mode","status":"merged"' "$release_trace" || {
    echo "ERROR: CORE-REL-003 full LTO did not emit merged trace evidence" >&2
    exit 1
}
EXTERNAL_PACKAGE_CONSUMER_FIXTURE=$(stage_external_package_fixture)
run_fixture external-package-consumer \
    "$EXTERNAL_PACKAGE_CONSUMER_FIXTURE"
run_fixture external-package-test \
    "$REPO_ROOT/tests/fixtures/external_package/tests/layout_contract.seen"
run_fixture lexical-semantic \
    "$REPO_ROOT/compiler_seen/tests/lexical_semantic.seen"
run_fixture declaration-frontend-smoke \
    "$REPO_ROOT/compiler_seen/tests/declaration_frontend_smoke.seen"
run_fixture generic-parameter-metadata \
    "$REPO_ROOT/compiler_seen/tests/generic_parameter_metadata.seen"
run_fixture language-pack-validation \
    "$REPO_ROOT/compiler_seen/tests/language_pack_validation.seen"
run_fixture formatter "$REPO_ROOT/compiler_seen/tests/lsp_formatter.seen"
run_fixture nullable-option-codegen \
    "$REPO_ROOT/compiler_seen/tests/nullable_option_codegen.seen"
run_fixture nullable-class-primitive-runtime \
    "$REPO_ROOT/compiler_seen/tests/nullable_class_primitive_runtime.seen"
run_fixture array-bool-push-codegen \
    "$REPO_ROOT/compiler_seen/tests/array_bool_push_codegen.seen"
"$REPO_ROOT/tests/misc_root_tests/seen_extern_runtime_declaration_dedup.sh"
"$REPO_ROOT/tests/misc_root_tests/seen_semantic_foundation.sh"

if [ "$TIER" = "verify" ]; then
    "$REPO_ROOT/tests/misc_root_tests/seen_atomic_text_io.sh"
    "$REPO_ROOT/tests/misc_root_tests/seen_package_client_bridge.sh"
    "$REPO_ROOT/tests/misc_root_tests/seen_linux_installer_handshake.sh"
    "$REPO_ROOT/tests/misc_root_tests/seen_editor_feature_parity.sh"

    GO_BIN="${SEEN_GO:-$(command -v go || true)}"
    [ -n "$GO_BIN" ] && [ -x "$GO_BIN" ] || {
        echo "ERROR: verify acceptance requires Go 1.26 via SEEN_GO or PATH" >&2
        exit 1
    }
    STAGE1_GO_CACHE=$(prepare_stage1_go_cache GOCACHE \
        "${GOCACHE:-$SEEN_ARTIFACT_ROOT/go-cache}")
    STAGE1_GO_MODCACHE=$(prepare_stage1_go_cache GOMODCACHE \
        "${GOMODCACHE:-$SEEN_ARTIFACT_ROOT/go-modcache}")
    STAGE1_GO_PATH=$(prepare_stage1_go_cache GOPATH \
        "$SEEN_ARTIFACT_ROOT/go-path")
    STAGE1_GO_TMP=$(prepare_stage1_go_cache GOTMPDIR \
        "$SEEN_ARTIFACT_ROOT/go-tmp")
    (
        cd "$REPO_ROOT/tools/seen-pkg"
        env GOCACHE="$STAGE1_GO_CACHE" GOMODCACHE="$STAGE1_GO_MODCACHE" \
            GOPATH="$STAGE1_GO_PATH" GOTMPDIR="$STAGE1_GO_TMP" \
            GOENV=off GOTELEMETRY=off GOTOOLCHAIN=local \
            GOMAXPROCS=1 GOFLAGS="-p=1 -modcacherw" \
            "$GO_BIN" test -p=1 ./...
    )

    bash "$REPO_ROOT/installer/test/test-runner.sh" --quick
    bash "$REPO_ROOT/installer/test/integration-test.sh"

    compile_fixture lsp-server \
        "$REPO_ROOT/compiler_seen/tests/lsp_server_main.seen"
    LSP_ARTIFACT_ROOT="$ACCEPTANCE_ROOT/lsp-jsonrpc"
    mkdir -p -- "$LSP_ARTIFACT_ROOT"
    SEEN_ARTIFACT_ROOT="$LSP_ARTIFACT_ROOT" \
        python3 "$REPO_ROOT/tests/lsp_formatter_jsonrpc.py" \
            "$ACCEPTANCE_ROOT/lsp-server" "$REPO_ROOT" "$ACCEPTANCE_ROOT"

    COMPILER="$REAL_COMPILER" \
        "$REPO_ROOT/tests/e2e_multilang/run_all_e2e.sh"
fi

echo "PASS: $TIER Stage-1 acceptance against $COMPILER"
