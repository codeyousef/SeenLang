#!/usr/bin/env bash
# Enter and re-verify the containment required by compiler-capable regressions.
#
# A standalone invocation first enters the project-local /tmp namespace and a
# read-back-verified aggregate cgroup.  Compiler execution is still refused
# unless the caller inherited the scope-bound fork-serializer attestation
# produced by safe_rebuild.sh.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
SELF_PATH="$SCRIPT_DIR/$(basename "${BASH_SOURCE[0]}")"
ARTIFACT_WRAPPER="$SCRIPT_DIR/run_with_project_artifacts.sh"
ARTIFACT_HELPER="$SCRIPT_DIR/artifact_root.sh"
HARD_SCOPE_WRAPPER="$SCRIPT_DIR/run_in_hard_memory_scope.sh"
SERIALIZER_VERIFY="$SCRIPT_DIR/verify_fork_serializer.sh"
BUILDER_APPLICABILITY="$SCRIPT_DIR/rebuild_builder_applicability.sh"
BUILDER_CAPABILITY="$SCRIPT_DIR/rebuild_builder_capability.sh"
BOUNDED_TOOLCHAIN_PREPARE="$SCRIPT_DIR/prepare_bounded_toolchain.sh"
ATTESTED_RUNNER="$SCRIPT_DIR/run_attested_seen.sh"
MAX_MAIN_KB=4194304
MAX_OPT_KB=2097152
MAX_TASKS=24
MODE=run

usage() {
    cat <<EOF
Usage:
  $0 <scope> --compiler <absolute-or-explicit-path> -- command [args...]
  $0 --verify-active <scope> --compiler <compiler>
  $0 --classify-active <scope> --compiler <compiler>
  $0 --platform <scope> -- command [args...]
  $0 --verify-platform-active <scope>

The run form re-enters project-local artifacts and a verified hard-memory
scope, validates an inherited fork-serializer attestation, prepares the
bounded toolchain, and then runs command with its argv unchanged.

The platform form provides the same artifact, aggregate-scope, and bounded-
toolchain containment for regressions that do not invoke Seen. It is Linux-
only and refuses before creating artifacts when no equivalent hard scope is
available.
EOF
}

fail() {
    echo "RESOURCE STOP: capped regression: $*" >&2
    exit 126
}

is_positive_integer() {
    case "${1:-}" in
        ''|*[!0-9]*) return 1 ;;
        *) [ "$1" -gt 0 ] 2>/dev/null ;;
    esac
}

canonicalize_compiler() {
    local requested=$1
    local candidate parent base canonical_parent

    case "$requested" in
        /*) candidate=$requested ;;
        */*) candidate="$PWD/$requested" ;;
        *) fail "PATH compiler names are forbidden; pass an explicit compiler path" ;;
    esac
    [ ! -L "$candidate" ] || fail "compiler path is a symbolic link: $candidate"
    parent=$(dirname -- "$candidate")
    base=$(basename -- "$candidate")
    canonical_parent=$(cd -P -- "$parent" 2>/dev/null && pwd -P) ||
        fail "compiler parent directory is not resolvable: $parent"
    candidate="$canonical_parent/$base"
    [ -f "$candidate" ] && [ -x "$candidate" ] && [ ! -L "$candidate" ] ||
        fail "compiler must be a regular non-symlink executable: $candidate"
    printf '%s\n' "$candidate"
}

validate_artifact_namespace() {
    local tmp_relative canonical_tmpdir namespace_identity artifact_identity

    [ -f "$ARTIFACT_HELPER" ] || fail "missing artifact-root helper"
    # shellcheck source=scripts/artifact_root.sh
    source "$ARTIFACT_HELPER"
    seen_artifact_root_init "$REPO_ROOT" || fail "artifact-root validation failed"
    [ "${SEEN_PROJECT_ARTIFACT_WRAPPER:-0}" = "1" ] &&
        [ "${SEEN_PROJECT_ARTIFACT_NAMESPACE_ACTIVE:-0}" = "1" ] ||
        fail "project-local artifact namespace markers are missing"
    [ -d "$SEEN_ARTIFACT_ROOT" ] && [ ! -L "$SEEN_ARTIFACT_ROOT" ] ||
        fail "artifact root is unsafe"

    case "${TMPDIR:-}" in
        "$SEEN_ARTIFACT_ROOT"/*) tmp_relative=${TMPDIR#"$REPO_ROOT"/} ;;
        *) fail "TMPDIR escaped the validated project artifact root" ;;
    esac
    seen_artifact_assert_safe_relative_path "$tmp_relative" ||
        fail "TMPDIR relative path is unsafe"
    seen_artifact_assert_no_symlink_components "$REPO_ROOT" "$tmp_relative" ||
        fail "TMPDIR traverses a symbolic link"
    [ -d "$TMPDIR" ] && [ ! -L "$TMPDIR" ] && [ -w "$TMPDIR" ] ||
        fail "TMPDIR is not a safe writable directory"
    canonical_tmpdir=$(seen_artifact_canonical_dir "$TMPDIR" || true)
    [ "$canonical_tmpdir" = "$TMPDIR" ] || fail "TMPDIR is not canonical"

    [ "$(uname -s)" = "Linux" ] ||
        fail "no verified non-Linux private-/tmp implementation is available"
    namespace_identity=$(stat -c '%d:%i' /tmp 2>/dev/null || true)
    artifact_identity=$(stat -c '%d:%i' "$SEEN_ARTIFACT_ROOT" 2>/dev/null || true)
    [ -n "$namespace_identity" ] && [ "$namespace_identity" = "$artifact_identity" ] ||
        fail "private /tmp does not map to the validated project artifact root"
}

validate_caps() {
    local active_main memory_limit_ceiling

    is_positive_integer "${SEEN_MEMORY_GUARD_RSS_KB:-}" ||
        fail "missing aggregate memory cap"
    is_positive_integer "${SEEN_MEMORY_GUARD_TASKS_MAX:-}" ||
        fail "missing aggregate task cap"
    is_positive_integer "${SEEN_MAIN_VMEM_KB:-}" || fail "missing main memory cap"
    is_positive_integer "${SEEN_OPT_VMEM_KB:-}" || fail "missing optimizer memory cap"
    is_positive_integer "${SEEN_MEMORY_LIMIT_BYTES:-}" ||
        fail "missing compiler allocation budget"

    [ "$SEEN_MEMORY_GUARD_RSS_KB" -le "$MAX_MAIN_KB" ] ||
        fail "aggregate memory cap exceeds 4 GiB"
    [ "$SEEN_MEMORY_GUARD_TASKS_MAX" -le "$MAX_TASKS" ] ||
        fail "aggregate task cap exceeds $MAX_TASKS"
    [ "$SEEN_MAIN_VMEM_KB" -le "$SEEN_MEMORY_GUARD_RSS_KB" ] &&
        [ "$SEEN_MAIN_VMEM_KB" -le "$MAX_MAIN_KB" ] ||
        fail "main memory cap exceeds the aggregate or 4 GiB ceiling"
    [ "$SEEN_OPT_VMEM_KB" -le "$SEEN_MAIN_VMEM_KB" ] &&
        [ "$SEEN_OPT_VMEM_KB" -le "$MAX_OPT_KB" ] ||
        fail "optimizer memory cap exceeds the main or 2 GiB ceiling"
    memory_limit_ceiling=$((SEEN_MAIN_VMEM_KB * 1024))
    [ "$SEEN_MEMORY_LIMIT_BYTES" -le "$memory_limit_ceiling" ] ||
        fail "compiler allocation budget exceeds the main cap"
    [ "${SEEN_LOW_MEMORY:-0}" = "1" ] &&
        [ "${SEEN_JOBS:-0}" = "1" ] && [ "${SEEN_OPT_JOBS:-0}" = "1" ] ||
        fail "serial low-memory execution settings are missing"

    if ! ulimit -S -v "$SEEN_MAIN_VMEM_KB" 2>/dev/null; then
        fail "could not apply the main virtual-memory cap"
    fi
    active_main=$(ulimit -S -v 2>/dev/null || true)
    is_positive_integer "$active_main" || fail "could not read back the main memory cap"
    [ "$active_main" -le "$SEEN_MAIN_VMEM_KB" ] ||
        fail "active main memory cap exceeds the requested cap"
}

verify_hard_scope() {
    local reentry=()

    [ -f "$HARD_SCOPE_WRAPPER" ] || fail "missing hard-memory-scope wrapper"
    if [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" != "1" ]; then
        case "$MODE" in
            run)
                reentry=(bash "$SELF_PATH" "$SCOPE" --compiler "$COMPILER" --)
                ;;
            platform-run)
                reentry=(bash "$SELF_PATH" --platform "$SCOPE" --)
                ;;
            *) fail "hard-scope entry was requested outside a run mode" ;;
        esac
        exec bash "$HARD_SCOPE_WRAPPER" \
            --label "Capped regression $SCOPE" -- \
            "${reentry[@]}" "${COMMAND[@]}"
    fi

    # A safe_rebuild aggregate scope uses its own active marker.  The verifier
    # below proves the inherited cgroup before this compatibility marker is used.
    SEEN_HARD_MEMORY_SCOPE_ACTIVE=1
    export SEEN_HARD_MEMORY_SCOPE_ACTIVE
    if ! bash "$HARD_SCOPE_WRAPPER" \
        --label "Capped regression $SCOPE read-back" --verify-only -- \
        >/dev/null; then

        fail "aggregate hard-memory scope read-back failed"
    fi
    validate_caps
}

verify_serializer_for_compiler() {
    local candidate=$1
    local serializer=${SEEN_FORK_SERIALIZER_SO:-}
    local attestation=${SEEN_FORK_SERIALIZER_ATTESTATION:-}
    local capability
    local capability_status=0

    [ -f "$SERIALIZER_VERIFY" ] || fail "missing serializer verifier"
    [ -f "$BUILDER_APPLICABILITY" ] || fail "missing serializer applicability checker"
    [ -f "$BUILDER_CAPABILITY" ] || fail "missing compiler schema classifier"
    if ! bash "$SERIALIZER_VERIFY" "$serializer" "$attestation" \
        "$SEEN_ARTIFACT_ROOT" "${SEEN_MEMORY_GUARD_SCOPE_UNIT:-}" \
        >/dev/null; then

        fail "inherited fork-serializer attestation is absent or invalid"
    fi
    if ! bash "$BUILDER_APPLICABILITY" "$candidate" "$serializer" >/dev/null; then
        fail "compiler cannot be contained by the attested serializer"
    fi

    capability=$(env -u LD_PRELOAD -u SEEN_FORK_SERIALIZER_TARGET \
        -u SEEN_FORK_SERIALIZER_ROOT_PID \
        bash "$BUILDER_CAPABILITY" "$candidate" 2>/dev/null) ||
        capability_status=$?
    if [ "$capability_status" -ne 0 ]; then
        fail "compiler worker-control schema classifier failed with status $capability_status"
    fi
    case "$capability" in
        advertised-jobs|advertised-no-fork|serializer-required) ;;
        *) fail "compiler worker-control schema classification failed" ;;
    esac
    printf '%s\n' "$capability"
}

validate_bounded_toolchain() {
    local bounded=${SEEN_BOUNDED_TOOLCHAIN_DIR:-}
    local canonical_bounded
    local expected_bounded="$SEEN_ARTIFACT_ROOT/bounded-toolchain"

    [ "$bounded" = "$expected_bounded" ] ||
        fail "bounded toolchain does not match the canonical prepared path"
    if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" = "1" ]; then
        [ "${SEEN_CAPPED_REGRESSION_TOOLCHAIN:-}" = "$bounded" ] ||
            fail "bounded toolchain does not match the active entry binding"
    fi
    if [ "${SEEN_CAPPED_PLATFORM_REGRESSION_ACTIVE:-0}" = "1" ]; then
        [ "${SEEN_CAPPED_PLATFORM_REGRESSION_TOOLCHAIN:-}" = "$bounded" ] ||
            fail "bounded toolchain does not match the active platform binding"
    fi
    [ -d "$bounded" ] && [ ! -L "$bounded" ] || fail "bounded toolchain is unsafe"
    canonical_bounded=$(seen_artifact_canonical_dir "$bounded" || true)
    [ "$canonical_bounded" = "$bounded" ] || fail "bounded toolchain is not canonical"
    case "${PATH:-}" in
        "$bounded"|"$bounded":*) ;;
        *) fail "bounded toolchain must be the first PATH entry" ;;
    esac
}

prepare_bounded_toolchain() {
    local bounded

    [ -f "$BOUNDED_TOOLCHAIN_PREPARE" ] || fail "missing bounded-toolchain helper"
    bounded=$(bash "$BOUNDED_TOOLCHAIN_PREPARE" "$SEEN_ARTIFACT_ROOT") ||
        fail "could not prepare the bounded optimizer/linker toolchain"
    case "$bounded" in
        "$SEEN_ARTIFACT_ROOT"/*) ;;
        *) fail "prepared toolchain escaped the artifact root" ;;
    esac
    SEEN_BOUNDED_TOOLCHAIN_DIR=$bounded
    PATH="$bounded${PATH:+:$PATH}"
    export SEEN_BOUNDED_TOOLCHAIN_DIR PATH
    validate_bounded_toolchain
}

validate_active_binding() {
    [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" = "1" ] ||
        fail "active capped-regression marker is missing"
    [ "${SEEN_CAPPED_REGRESSION_SCOPE:-}" = "$SCOPE" ] ||
        fail "active capped-regression scope does not match"
    [ "${SEEN_ATTESTED_COMPILER_RUNNER:-}" = "$ATTESTED_RUNNER" ] ||
        fail "attested compiler runner binding does not match"
}

validate_platform_binding() {
    [ "${SEEN_CAPPED_PLATFORM_REGRESSION_ACTIVE:-0}" = "1" ] ||
        fail "active capped-platform marker is missing"
    [ "${SEEN_CAPPED_PLATFORM_REGRESSION_SCOPE:-}" = "$SCOPE" ] ||
        fail "active capped-platform scope does not match"
}

case "${1:-}" in
    -h|--help)
        usage
        exit 0
        ;;
    --verify-active)
        MODE=verify
        shift
        ;;
    --classify-active)
        MODE=classify
        shift
        ;;
    --platform)
        MODE=platform-run
        shift
        ;;
    --verify-platform-active)
        MODE=platform-verify
        shift
        ;;
esac

[ "$#" -ge 1 ] || {
    usage >&2
    exit 2
}
SCOPE=$1
shift
case "$SCOPE" in
    ''|*[!A-Za-z0-9._-]*) fail "invalid artifact scope name" ;;
esac
COMPILER=""
case "$MODE" in
    run|verify|classify)
        [ "${1:-}" = "--compiler" ] && [ "$#" -ge 2 ] || {
            usage >&2
            exit 2
        }
        shift
        COMPILER=$(canonicalize_compiler "$1")
        shift
        ;;
    platform-run|platform-verify) ;;
    *) fail "unknown capped-regression mode" ;;
esac
COMMAND=()

if [ "$MODE" = "run" ] || [ "$MODE" = "platform-run" ]; then
    [ "${1:-}" = "--" ] || {
        usage >&2
        exit 2
    }
    shift
    [ "$#" -gt 0 ] || fail "missing regression command"
    COMMAND=("$@")
else
    [ "$#" -eq 0 ] || fail "unexpected operands in verification mode"
fi

case "$MODE" in
    platform-*)
        [ "$(uname -s)" = "Linux" ] ||
            fail "no equivalent non-Linux aggregate hard scope is available"
        ;;
esac

if { [ "$MODE" = "run" ] || [ "$MODE" = "platform-run" ]; } &&
    { [ "${SEEN_PROJECT_ARTIFACT_WRAPPER:-0}" != "1" ] ||
      [ "${SEEN_PROJECT_ARTIFACT_NAMESPACE_ACTIVE:-0}" != "1" ]; }; then

    artifact_args=("$SCOPE")
    if [ -n "${SEEN_KEEP_TMP:-}" ]; then
        artifact_args+=(--keep-on-failure)
    fi
    if [ "$MODE" = "run" ]; then
        artifact_args+=(-- bash "$SELF_PATH" "$SCOPE" --compiler "$COMPILER" --)
    else
        artifact_args+=(-- bash "$SELF_PATH" --platform "$SCOPE" --)
    fi
    artifact_args+=("${COMMAND[@]}")
    exec bash "$ARTIFACT_WRAPPER" "${artifact_args[@]}"
fi

validate_artifact_namespace
cd -- "$REPO_ROOT" || fail "could not enter the repository root"
if [ "$MODE" = "run" ] || [ "$MODE" = "platform-run" ]; then
    verify_hard_scope
else
    [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" = "1" ] ||
        fail "verified aggregate scope marker is missing"
    SEEN_HARD_MEMORY_SCOPE_ACTIVE=1
    export SEEN_HARD_MEMORY_SCOPE_ACTIVE
    if ! bash "$HARD_SCOPE_WRAPPER" \
        --label "Capped regression $SCOPE read-back" --verify-only -- \
        >/dev/null; then

        fail "aggregate hard-memory scope read-back failed"
    fi
    validate_caps
fi

if [ "$MODE" = "verify" ] || [ "$MODE" = "classify" ]; then
    validate_active_binding
    validate_bounded_toolchain
fi
if [ "$MODE" = "platform-verify" ]; then
    validate_platform_binding
    validate_bounded_toolchain
    exit 0
fi

if [ "$MODE" = "verify" ]; then
    [ "${SEEN_CAPPED_REGRESSION_COMPILER:-}" = "$COMPILER" ] ||
        fail "active compiler binding does not match"
    verify_serializer_for_compiler "$COMPILER" >/dev/null
    exit 0
fi

if [ "$MODE" = "classify" ]; then
    verify_serializer_for_compiler "$COMPILER"
    exit 0
fi

if [ "$MODE" = "run" ]; then
    verify_serializer_for_compiler "$COMPILER" >/dev/null
fi
prepare_bounded_toolchain
if [ "$MODE" = "platform-run" ]; then
    SEEN_CAPPED_PLATFORM_REGRESSION_ACTIVE=1
    SEEN_CAPPED_PLATFORM_REGRESSION_SCOPE=$SCOPE
    SEEN_CAPPED_PLATFORM_REGRESSION_TOOLCHAIN=$SEEN_BOUNDED_TOOLCHAIN_DIR
    export SEEN_CAPPED_PLATFORM_REGRESSION_ACTIVE
    export SEEN_CAPPED_PLATFORM_REGRESSION_SCOPE
    export SEEN_CAPPED_PLATFORM_REGRESSION_TOOLCHAIN
    unset LD_PRELOAD
    exec "${COMMAND[@]}"
fi
SEEN_CAPPED_REGRESSION_ACTIVE=1
SEEN_CAPPED_REGRESSION_SCOPE=$SCOPE
SEEN_CAPPED_REGRESSION_COMPILER=$COMPILER
SEEN_CAPPED_REGRESSION_TOOLCHAIN=$SEEN_BOUNDED_TOOLCHAIN_DIR
SEEN_ATTESTED_COMPILER_RUNNER=$ATTESTED_RUNNER
export SEEN_CAPPED_REGRESSION_ACTIVE SEEN_CAPPED_REGRESSION_SCOPE
export SEEN_CAPPED_REGRESSION_COMPILER SEEN_CAPPED_REGRESSION_TOOLCHAIN
export SEEN_ATTESTED_COMPILER_RUNNER
unset LD_PRELOAD
exec "${COMMAND[@]}"
