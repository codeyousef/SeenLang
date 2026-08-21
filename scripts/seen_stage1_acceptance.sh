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
EXTERNAL_PACKAGE_CONSUMER_FIXTURE=$(stage_external_package_fixture)
run_fixture external-package-consumer \
    "$EXTERNAL_PACKAGE_CONSUMER_FIXTURE"
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
