#!/bin/bash
# Safe rebuild: Only updates compiler if bootstrap verifies
#
# This script builds a new compiler from the frozen bootstrap and only
# installs compiler artifacts that pass smoke tests.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
ARTIFACT_ROOT_SCRIPT="$SCRIPT_DIR/artifact_root.sh"
SERIAL_AUXILIARY_SCRIPT="$SCRIPT_DIR/serial_auxiliary_env.sh"
SAFE_REBUILD_ORIGINAL_ARGS=("$@")

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
NC='\033[0m' # No Color

STAGE2=""
STAGE3=""
STAGE3_RECOVERY=""
PRESERVED_PROD_BUILDER=""
REBUILD_ARTIFACT_SCOPE=""
REBUILD_WORK_ROOT=""
COMPILER_ARTIFACT_ROOT=""
# PROJECT_ARTIFACT_ROOT is the stable project-wide base. Preserve it across
# both the private-/tmp re-exec and the later hard-cgroup re-exec; the latter
# intentionally carries SEEN_ARTIFACT_ROOT as the per-run directory.
PROJECT_ARTIFACT_ROOT="${PROJECT_ARTIFACT_ROOT:-}"
COMPILER_SOURCE="compiler_seen/src/main_compiler.seen"
MEMORY_GUARD_SCRIPT="$SCRIPT_DIR/memory_guard.sh"
HARD_MEMORY_SCOPE_WRAPPER="$SCRIPT_DIR/run_in_hard_memory_scope.sh"
BUILDER_CAPABILITY_SCRIPT="$SCRIPT_DIR/rebuild_builder_capability.sh"
BUILDER_APPLICABILITY_SCRIPT="$SCRIPT_DIR/rebuild_builder_applicability.sh"
BUILDER_SELECTION_SCRIPT="$SCRIPT_DIR/rebuild_builder_selection.sh"
FORK_SERIALIZER_SOURCE="$SCRIPT_DIR/fork_serializer.c"
FORK_SERIALIZER_SELFTEST_SOURCE="$SCRIPT_DIR/fork_serializer_selftest.c"
FORK_SERIALIZER_SO=""
FORK_SERIALIZER_ATTESTATION=""
FORK_SERIALIZER_VERIFY_SCRIPT="$SCRIPT_DIR/verify_fork_serializer.sh"
BOUNDED_TOOLCHAIN_PREPARE_SCRIPT="$SCRIPT_DIR/prepare_bounded_toolchain.sh"
BOUNDED_TOOLCHAIN_DIR=""
BOOTSTRAP_SOURCE_ROOT=""
BOOTSTRAP_PREFLIGHT_DONE=0
FROZEN_ABS=""
BUILD_TRACE_COMMON="$SCRIPT_DIR/build_trace_common.sh"
REBUILD_TIER="full"
CLEAN_CACHE=0
ARTIFACT_PREFLIGHT_ONLY=0
PACKAGE_CLIENT_BUILD_OUTPUT="$REPO_ROOT/target/seen-build/package-client/seen-pkg"
SOURCE_PACKAGE_CLIENT=""
SOURCE_PACKAGE_CLIENT_VERSION=""
REBUILD_SCOPE_EVIDENCE_FILE=""

if [ -f "$BUILD_TRACE_COMMON" ]; then
    # shellcheck source=scripts/build_trace_common.sh
    source "$BUILD_TRACE_COMMON"
fi

safe_rebuild_usage() {
    echo "Usage: $0 [--tier quick|verify|full] [--clean-cache] [--artifact-preflight] [--help]"
    echo ""
    echo "Tiers:"
    echo "  quick   Cache-enabled developer rebuild to compiler_seen/target/seen-dev; smoke only."
    echo "  verify  Cache-enabled production rebuild; targeted checks; install after verification."
    echo "  full    Cold staged bootstrap verification. This is the default for compatibility."
    echo ""
    echo "Artifacts default to <repository>/.seen/agent-tools. Set SEEN_ARTIFACT_ROOT"
    echo "to another Git-ignored path inside the repository to override that location."
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --tier)
            if [ "$#" -lt 2 ]; then
                echo -e "${RED:-}ERROR: --tier requires quick, verify, or full.${NC:-}" >&2
                exit 1
            fi
            REBUILD_TIER="$2"
            shift 2
            ;;
        --tier=*)
            REBUILD_TIER="${1#--tier=}"
            shift
            ;;
        --clean-cache)
            CLEAN_CACHE=1
            shift
            ;;
        --artifact-preflight)
            ARTIFACT_PREFLIGHT_ONLY=1
            shift
            ;;
        -h|--help)
            safe_rebuild_usage
            exit 0
            ;;
        *)
            echo -e "${RED:-}ERROR: unknown safe rebuild option: $1${NC:-}" >&2
            safe_rebuild_usage >&2
            exit 1
            ;;
    esac
done

case "$REBUILD_TIER" in
    quick|verify|full) ;;
    *)
        echo -e "${RED:-}ERROR: --tier must be quick, verify, or full.${NC:-}" >&2
        exit 1
        ;;
esac

if [ ! -f "$ARTIFACT_ROOT_SCRIPT" ]; then
    echo -e "${RED:-}ERROR: missing artifact-root helper: $ARTIFACT_ROOT_SCRIPT${NC:-}" >&2
    exit 1
fi
if [ ! -f "$SERIAL_AUXILIARY_SCRIPT" ]; then
    echo -e "${RED:-}ERROR: missing serial-auxiliary helper: $SERIAL_AUXILIARY_SCRIPT${NC:-}" >&2
    exit 1
fi
# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_ROOT_SCRIPT"
# shellcheck source=scripts/serial_auxiliary_env.sh
source "$SERIAL_AUXILIARY_SCRIPT"
if [ "${SEEN_ARTIFACT_NAMESPACE_ACTIVE:-0}" = "1" ]; then
    inherited_project_artifact_root=$PROJECT_ARTIFACT_ROOT
    case "$inherited_project_artifact_root" in
        "$REPO_ROOT"/*) ;;
        *)
            echo -e "${RED:-}ERROR: inherited project artifact root is missing or escaped the repository: $inherited_project_artifact_root${NC:-}" >&2
            exit 1
            ;;
    esac
    if [ ! -d "$inherited_project_artifact_root" ] ||
        [ -L "$inherited_project_artifact_root" ]; then

        echo -e "${RED:-}ERROR: inherited project artifact root is unsafe: $inherited_project_artifact_root${NC:-}" >&2
        exit 1
    fi
    canonical_inherited_project_root=$(seen_artifact_canonical_dir \
        "$inherited_project_artifact_root") || exit 1
    if [ "$canonical_inherited_project_root" != "$inherited_project_artifact_root" ]; then
        echo -e "${RED:-}ERROR: inherited project artifact root is not canonical: $inherited_project_artifact_root${NC:-}" >&2
        exit 1
    fi
    # Restore the stable base before resolving the safe-rebuild scope. Later,
    # SEEN_ARTIFACT_ROOT is deliberately rebound to REBUILD_WORK_ROOT for tools.
    SEEN_ARTIFACT_ROOT=$inherited_project_artifact_root
    export SEEN_ARTIFACT_ROOT PROJECT_ARTIFACT_ROOT
fi
seen_artifact_root_init "$REPO_ROOT" || exit 1
PROJECT_ARTIFACT_ROOT="$SEEN_ARTIFACT_ROOT"
REBUILD_ARTIFACT_SCOPE=$(seen_artifact_scope_init safe-rebuild) || exit 1
if [ "${SEEN_ARTIFACT_NAMESPACE_ACTIVE:-0}" = "1" ]; then
    REBUILD_WORK_ROOT=${SEEN_REBUILD_WORK_ROOT:-}
    case "$REBUILD_WORK_ROOT" in
        "$REBUILD_ARTIFACT_SCOPE"/*)
            inherited_rebuild_name=${REBUILD_WORK_ROOT#"$REBUILD_ARTIFACT_SCOPE"/}
            ;;
        *)
            echo -e "${RED:-}ERROR: invalid inherited rebuild artifact directory: $REBUILD_WORK_ROOT${NC:-}" >&2
            exit 1
            ;;
    esac
    case "$inherited_rebuild_name" in
        preflight|run.*) ;;
        *)
            echo -e "${RED:-}ERROR: invalid inherited rebuild artifact basename: $inherited_rebuild_name${NC:-}" >&2
            exit 1
            ;;
    esac
    case "$inherited_rebuild_name" in
        */*)
            echo -e "${RED:-}ERROR: inherited rebuild artifact directory may not contain nested components.${NC:-}" >&2
            exit 1
            ;;
    esac
    inherited_rebuild_relative=${REBUILD_WORK_ROOT#"$REPO_ROOT"/}
    seen_artifact_assert_safe_relative_path "$inherited_rebuild_relative" || exit 1
    seen_artifact_assert_no_symlink_components "$REPO_ROOT" \
        "$inherited_rebuild_relative" || exit 1
    if [ ! -d "$REBUILD_WORK_ROOT" ] || [ -L "$REBUILD_WORK_ROOT" ]; then
        echo -e "${RED:-}ERROR: inherited rebuild artifact directory is unsafe: $REBUILD_WORK_ROOT${NC:-}" >&2
        exit 1
    fi
else
    if [ "$ARTIFACT_PREFLIGHT_ONLY" = "1" ]; then
        REBUILD_WORK_ROOT="$REBUILD_ARTIFACT_SCOPE/preflight"
        if [ -L "$REBUILD_WORK_ROOT" ]; then
            echo -e "${RED:-}ERROR: artifact preflight directory is a symbolic link: $REBUILD_WORK_ROOT${NC:-}" >&2
            exit 1
        fi
        preflight_rebuild_relative=${REBUILD_WORK_ROOT#"$REPO_ROOT"/}
        seen_artifact_assert_safe_relative_path "$preflight_rebuild_relative" || exit 1
        seen_artifact_assert_no_symlink_components "$REPO_ROOT" \
            "$preflight_rebuild_relative" || exit 1
        mkdir -p -- "$REBUILD_WORK_ROOT" || exit 1
    else
        REBUILD_WORK_ROOT=$(seen_artifact_mktemp_dir "$REBUILD_ARTIFACT_SCOPE" run) || exit 1
    fi
fi
rebuild_work_relative=${REBUILD_WORK_ROOT#"$REPO_ROOT"/}
case "$REBUILD_WORK_ROOT" in
    "$REPO_ROOT"/*) ;;
    *)
        echo -e "${RED:-}ERROR: rebuild artifact directory escaped the repository.${NC:-}" >&2
        exit 1
        ;;
esac
seen_artifact_assert_safe_relative_path "$rebuild_work_relative" || exit 1
seen_artifact_assert_no_symlink_components "$REPO_ROOT" "$rebuild_work_relative" || exit 1
canonical_rebuild_work_root=$(seen_artifact_canonical_dir "$REBUILD_WORK_ROOT") || exit 1
if [ "$canonical_rebuild_work_root" != "$REBUILD_WORK_ROOT" ]; then
    echo -e "${RED:-}ERROR: rebuild artifact directory is not canonical: $REBUILD_WORK_ROOT${NC:-}" >&2
    exit 1
fi

# Production outputs are not disposable artifact-root entries, but they are
# still write boundaries. Reject poisoned parent components and endpoints, and
# install through a same-directory temporary file so an existing hard link is
# replaced rather than truncated in place.
safe_rebuild_prepare_checkout_directory() {
    local relative_dir=$1
    local destination canonical_destination expected_destination

    seen_artifact_assert_safe_relative_path "$relative_dir" || return 1
    seen_artifact_assert_no_symlink_components "$REPO_ROOT" \
        "$relative_dir" || return 1
    if [ "$relative_dir" = "." ]; then
        destination=$REPO_ROOT
        expected_destination=$REPO_ROOT
    else
        destination="$REPO_ROOT/$relative_dir"
        expected_destination=$destination
    fi
    if [ -e "$destination" ] && [ ! -d "$destination" ]; then
        echo -e "${RED:-}ERROR: install parent is not a directory: $destination${NC:-}" >&2
        return 1
    fi
    [ ! -L "$destination" ] || {
        echo -e "${RED:-}ERROR: install parent is a symbolic link: $destination${NC:-}" >&2
        return 1
    }
    mkdir -p -- "$destination" || return 1
    seen_artifact_assert_no_symlink_components "$REPO_ROOT" \
        "$relative_dir" || return 1
    canonical_destination=$(seen_artifact_canonical_dir "$destination") || return 1
    if [ "$canonical_destination" != "$expected_destination" ]; then
        echo -e "${RED:-}ERROR: install parent is not canonical: $destination${NC:-}" >&2
        return 1
    fi
}

safe_rebuild_assert_checkout_output() {
    local relative_path=$1
    local parent target

    seen_artifact_assert_safe_relative_path "$relative_path" || return 1
    case "$relative_path" in
        */*) parent=${relative_path%/*} ;;
        *) parent=. ;;
    esac
    safe_rebuild_prepare_checkout_directory "$parent" || return 1
    target="$REPO_ROOT/$relative_path"
    if [ -L "$target" ]; then
        echo -e "${RED:-}ERROR: refusing symbolic-link install target: $target${NC:-}" >&2
        return 1
    fi
    if [ -e "$target" ] && [ ! -f "$target" ]; then
        echo -e "${RED:-}ERROR: install target is not a regular file: $target${NC:-}" >&2
        return 1
    fi
}

safe_rebuild_install_checkout_file() {
    local source=$1
    local relative_path=$2
    local target parent base temporary

    if [ ! -f "$source" ] || [ -L "$source" ]; then
        echo -e "${RED:-}ERROR: install source is not a regular non-symlink file: $source${NC:-}" >&2
        return 1
    fi
    safe_rebuild_assert_checkout_output "$relative_path" || return 1
    target="$REPO_ROOT/$relative_path"
    parent=$(dirname -- "$target")
    base=$(basename -- "$target")
    temporary=$(mktemp "$parent/.${base}.install.XXXXXX") || return 1
    if ! cp -- "$source" "$temporary" || ! chmod 755 "$temporary"; then
        rm -f -- "$temporary"
        return 1
    fi
    if ! mv -f -- "$temporary" "$target"; then
        rm -f -- "$temporary"
        return 1
    fi
    [ -f "$target" ] && [ ! -L "$target" ] || {
        echo -e "${RED:-}ERROR: installed output is unsafe: $target${NC:-}" >&2
        return 1
    }
}

safe_rebuild_remove_checkout_file() {
    local relative_path=$1
    local target

    safe_rebuild_assert_checkout_output "$relative_path" || return 1
    target="$REPO_ROOT/$relative_path"
    rm -f -- "$target"
}

safe_rebuild_validate_install_destinations() {
    local relative_path
    for relative_path in \
        compiler_seen/target/seen \
        compiler_seen/target/seen-dev \
        compiler_seen/target/seen-pkg \
        target/release/seen \
        target/release/seen-pkg \
        stage2_head stage3_head stage3_recovery_head; do

        safe_rebuild_assert_checkout_output "$relative_path" || return 1
    done
}
COMPILER_ARTIFACT_ROOT="$REBUILD_WORK_ROOT"
if [ -L "$COMPILER_ARTIFACT_ROOT" ]; then
    echo -e "${RED:-}ERROR: compiler artifact scope is a symbolic link: $COMPILER_ARTIFACT_ROOT${NC:-}" >&2
    exit 1
fi
mkdir -p -- "$COMPILER_ARTIFACT_ROOT" "$REBUILD_WORK_ROOT/tool-tmp"
if [ ! -d "$COMPILER_ARTIFACT_ROOT" ] || [ -L "$COMPILER_ARTIFACT_ROOT" ]; then
    echo -e "${RED:-}ERROR: compiler artifact scope is not a safe directory: $COMPILER_ARTIFACT_ROOT${NC:-}" >&2
    exit 1
fi
TMPDIR="$REBUILD_WORK_ROOT/tool-tmp"
SEEN_ARTIFACT_ROOT="$COMPILER_ARTIFACT_ROOT"
export TMPDIR SEEN_ARTIFACT_ROOT PROJECT_ARTIFACT_ROOT

# Frozen 0.10 builders use absolute /tmp paths internally. On Linux, run the
# whole rebuild in a private mount namespace whose /tmp is this project-local
# per-run directory. This keeps the recovery logic intact without writing to the
# host's temporary filesystem. Other hosts must explicitly opt into the legacy
# behavior until their frozen builder understands SEEN_ARTIFACT_ROOT.
if [ "${SEEN_ARTIFACT_NAMESPACE_ACTIVE:-0}" != "1" ]; then
    artifact_host_os=$(uname -s)
    if [ "$artifact_host_os" = "Linux" ]; then
        if ! command -v bwrap >/dev/null 2>&1; then
            echo -e "${RED:-}ERROR: project-local frozen-bootstrap artifacts require bwrap on Linux.${NC:-}" >&2
            echo "Install Bubblewrap; this script will not fall back to host /tmp on Linux." >&2
            exit 1
        fi
        if ! bwrap --die-with-parent --bind / / --dev-bind /dev /dev \
            --proc /proc --ro-bind /sys /sys \
            --bind "$REBUILD_WORK_ROOT" /tmp -- true; then
            echo -e "${RED:-}ERROR: could not map the project artifact directory onto the rebuild's /tmp.${NC:-}" >&2
            exit 1
        fi
        export SEEN_ARTIFACT_NAMESPACE_ACTIVE=1
        export SEEN_PROJECT_ARTIFACT_WRAPPER=1
        export SEEN_PROJECT_ARTIFACT_NAMESPACE_ACTIVE=1
        export SEEN_REBUILD_WORK_ROOT="$REBUILD_WORK_ROOT"
        export SEEN_ARTIFACT_ROOT="$PROJECT_ARTIFACT_ROOT"
        exec bwrap --die-with-parent --bind / / --dev-bind /dev /dev \
            --proc /proc --ro-bind /sys /sys \
            --bind "$REBUILD_WORK_ROOT" /tmp -- \
            "$SCRIPT_DIR/safe_rebuild.sh" "${SAFE_REBUILD_ORIGINAL_ARGS[@]}"
    elif [ "${SEEN_ALLOW_SYSTEM_TMP:-0}" != "1" ]; then
        echo -e "${RED:-}ERROR: this frozen bootstrap cannot redirect absolute temporary paths on $artifact_host_os.${NC:-}" >&2
        echo "Set SEEN_ALLOW_SYSTEM_TMP=1 only if legacy host temporary-file use is acceptable." >&2
        exit 1
    else
        echo -e "${YELLOW:-}WARNING: SEEN_ALLOW_SYSTEM_TMP=1 permits legacy host temporary-file use.${NC:-}" >&2
    fi
else
    if [ "$(uname -s)" = "Linux" ]; then
        namespace_tmp_identity=$(stat -c '%d:%i' /tmp 2>/dev/null || true)
        work_root_identity=$(stat -c '%d:%i' "$REBUILD_WORK_ROOT" 2>/dev/null || true)
        if [ -z "$namespace_tmp_identity" ] || [ "$namespace_tmp_identity" != "$work_root_identity" ]; then
            echo -e "${RED:-}ERROR: rebuild artifact namespace validation failed.${NC:-}" >&2
            exit 1
        fi
    fi
fi
if ! cd -- "$REPO_ROOT"; then
    echo -e "${RED:-}ERROR: could not enter the canonical repository root: $REPO_ROOT${NC:-}" >&2
    exit 1
fi
SEEN_PROJECT_ARTIFACT_WRAPPER=1
SEEN_PROJECT_ARTIFACT_NAMESPACE_ACTIVE=1
export SEEN_PROJECT_ARTIFACT_WRAPPER SEEN_PROJECT_ARTIFACT_NAMESPACE_ACTIVE

# Only the outer aggregate supervisor owns its private `.aggregate-supervisor`
# TMPDIR. Nested command guards share the per-run tool-tmp directory and must
# never inherit permission to remove it.
SEEN_MEMORY_GUARD_REMOVE_EMPTY_TMPDIR=0
export SEEN_MEMORY_GUARD_REMOVE_EMPTY_TMPDIR

if [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" = "1" ]; then
    # A containing gate may already have prepared serial-tool state below its
    # own artifact root. This rebuild intentionally rebinds SEEN_ARTIFACT_ROOT
    # to its unique per-run directory before any compiler work, so materialize
    # the same fixed one-line policy at the new validated location and read it
    # back there. The live aggregate cgroup is independently re-verified below.
    seen_serial_auxiliary_prepare "$REPO_ROOT" "$SEEN_ARTIFACT_ROOT" || exit 126
    seen_serial_auxiliary_verify "$REPO_ROOT" "$SEEN_ARTIFACT_ROOT" || exit 126
else
    seen_serial_auxiliary_prepare "$REPO_ROOT" "$SEEN_ARTIFACT_ROOT" || exit 126
fi

# Do not forward caller-controlled metrics destinations into nested guards.
# Aggregate and per-step metrics are assigned explicitly below the validated
# project-local run root.
unset SEEN_MEMORY_GUARD_METRICS_FILE SEEN_MEMORY_GUARD_SUCCESS_METRICS_FILE

# Trace initialization can create its destination. Defer it until the private
# project-local temporary namespace is active so an explicit /tmp trace path
# cannot touch the host temporary filesystem during the initial re-exec.
if declare -F seen_build_trace_init >/dev/null 2>&1; then
    if [ -n "${SEEN_TRACE_BUILD:-}" ] || [ -n "${SEEN_BUILD_TRACE:-}" ]; then
        SEEN_TRACE_BUILD="$REBUILD_WORK_ROOT/safe-rebuild.trace.jsonl"
        SEEN_BUILD_TRACE="$SEEN_TRACE_BUILD"
        export SEEN_TRACE_BUILD SEEN_BUILD_TRACE
    fi
    seen_build_trace_init "safe_rebuild"
fi

if [ "$ARTIFACT_PREFLIGHT_ONLY" = "1" ]; then
    echo "Project artifact root: $PROJECT_ARTIFACT_ROOT"
    echo "Rebuild artifact directory: $REBUILD_WORK_ROOT"
    echo "Checkout working directory: $PWD"
    if [ "$(uname -s)" = "Linux" ]; then
        echo "Frozen-bootstrap temporary mapping: project-local"
    else
        echo "Frozen-bootstrap temporary mapping: legacy host temporary directory allowed"
    fi
    exit 0
fi

STAGE2="$REBUILD_WORK_ROOT/stage2_safe_rebuild"
STAGE3="$REBUILD_WORK_ROOT/stage3_safe_rebuild"
STAGE3_RECOVERY="$REBUILD_WORK_ROOT/stage3_safe_rebuild_recovery"
PRESERVED_PROD_BUILDER="$REBUILD_WORK_ROOT/seen_preserved_prod_builder"
REBUILD_FATAL_STATUS_FILE="$REBUILD_WORK_ROOT/.fatal-containment-status"
REBUILD_RECORDED_FATAL_STATUS=""
if [ -L "$REBUILD_FATAL_STATUS_FILE" ] || [ -e "$REBUILD_FATAL_STATUS_FILE" ]; then
    echo -e "${RED:-}ERROR: refusing pre-existing rebuild fatal-status marker.${NC:-}" >&2
    exit 1
fi

read_rebuild_fatal_status() {
    local recorded_status=""
    local extra_line=""
    local fatal_status_fd=""
    REBUILD_RECORDED_FATAL_STATUS=""
    [ "$REBUILD_FATAL_STATUS_FILE" = \
        "$REBUILD_WORK_ROOT/.fatal-containment-status" ] || return 1
    [ -f "$REBUILD_FATAL_STATUS_FILE" ] &&
        [ ! -L "$REBUILD_FATAL_STATUS_FILE" ] || return 1
    exec {fatal_status_fd}< "$REBUILD_FATAL_STATUS_FILE" || return 1
    IFS= read -r recorded_status <&"$fatal_status_fd" || {
        exec {fatal_status_fd}<&-
        return 1
    }
    if IFS= read -r extra_line <&"$fatal_status_fd"; then
        exec {fatal_status_fd}<&-
        return 1
    fi
    exec {fatal_status_fd}<&-
    case "$recorded_status" in
        124|125|126|137|143)
            REBUILD_RECORDED_FATAL_STATUS=$recorded_status
            ;;
        *) return 1 ;;
    esac
}

record_rebuild_fatal_status() {
    local fatal_status=$1
    local marker_status=1
    local noclobber_was_set=0
    case "$fatal_status" in
        124|125|126|137|143) ;;
        *) fatal_status=143 ;;
    esac
    [ "$REBUILD_FATAL_STATUS_FILE" = \
        "$REBUILD_WORK_ROOT/.fatal-containment-status" ] || return 1
    [ ! -L "$REBUILD_FATAL_STATUS_FILE" ] || return 1
    if [ -e "$REBUILD_FATAL_STATUS_FILE" ]; then
        read_rebuild_fatal_status
        return
    fi
    [[ -o noclobber ]] && noclobber_was_set=1
    set -C
    if printf '%s\n' "$fatal_status" > "$REBUILD_FATAL_STATUS_FILE" \
        2>/dev/null; then

        marker_status=0
    fi
    [ "$noclobber_was_set" -eq 1 ] || set +C
    return "$marker_status"
}

safe_rebuild_handle_term() {
    local fatal_status=143
    trap - TERM
    read_rebuild_fatal_status 2>/dev/null || true
    case "$REBUILD_RECORDED_FATAL_STATUS" in
        124|125|126|137|143)
            fatal_status=$REBUILD_RECORDED_FATAL_STATUS
            ;;
    esac
    exit "$fatal_status"
}

safe_rebuild_cleanup() {
    local status=$?
    local cleanup_entry=""
    local supervisor_dir="$REBUILD_WORK_ROOT/.aggregate-supervisor"
    if [ -e "$REBUILD_FATAL_STATUS_FILE" ] ||
        [ -L "$REBUILD_FATAL_STATUS_FILE" ]; then

        read_rebuild_fatal_status 2>/dev/null || true
        case "$REBUILD_RECORDED_FATAL_STATUS" in
            124|125|126|137|143)
                status=$REBUILD_RECORDED_FATAL_STATUS
                ;;
            *) status=143 ;;
        esac
    fi
    cleanup_bootstrap_source_overlay
    if declare -F seen_build_trace_summary >/dev/null 2>&1; then
        seen_build_trace_summary
    fi
    if [ "$status" -eq 0 ] && [ -n "$REBUILD_WORK_ROOT" ] &&
        [ -n "$REBUILD_ARTIFACT_SCOPE" ]; then
        case "$REBUILD_WORK_ROOT" in
            "$REBUILD_ARTIFACT_SCOPE"/run.*)
                if [ -d "$REBUILD_WORK_ROOT" ] && [ ! -L "$REBUILD_WORK_ROOT" ] &&
                    [ "$(dirname -- "$REBUILD_WORK_ROOT")" = "$REBUILD_ARTIFACT_SCOPE" ]; then
                    # The outer aggregate guard keeps live reason/ready state in
                    # this reserved directory until after the child exits and
                    # final metrics are retained. Clean every other direct entry,
                    # but never delete the supervisor's state out from under it.
                    for cleanup_entry in \
                        "$REBUILD_WORK_ROOT"/* \
                        "$REBUILD_WORK_ROOT"/.[!.]* \
                        "$REBUILD_WORK_ROOT"/..?*; do

                        [ -e "$cleanup_entry" ] || [ -L "$cleanup_entry" ] || continue
                        [ "$cleanup_entry" != "$supervisor_dir" ] || continue
                        [ "$(dirname -- "$cleanup_entry")" = "$REBUILD_WORK_ROOT" ] || {
                            status=1
                            break
                        }
                        rm -rf -- "$cleanup_entry"
                    done
                fi
                ;;
        esac
    fi
    return "$status"
}

# --- Progress monitoring helpers ---

# Format seconds as HH:MM:SS
format_time() {
    local secs=$1
    printf "%02d:%02d:%02d" $((secs/3600)) $(((secs%3600)/60)) $((secs%60))
}

# Format bytes as human-readable (pure bash, no bc dependency)
format_bytes() {
    local bytes=$1
    if [ "$bytes" -ge 1073741824 ]; then
        local gb=$((bytes / 1073741824))
        local remainder=$(( (bytes % 1073741824) * 10 / 1073741824 ))
        printf "%d.%dGB" "$gb" "$remainder"
    elif [ "$bytes" -ge 1048576 ]; then
        printf "%dMB" "$((bytes / 1048576))"
    elif [ "$bytes" -ge 1024 ]; then
        printf "%dKB" "$((bytes / 1024))"
    else
        printf "%dB" "$bytes"
    fi
}

latest_plain_module_ll() {
    local dir=$1
    local latest=""
    for f in "$dir"/seen_module_*.ll; do
        [ -f "$f" ] || continue
        [[ "$f" == *.opt.ll ]] && continue
        [[ "$f" == *.polly.ll ]] && continue
        if [ -z "$latest" ] || [ "$f" -nt "$latest" ]; then
            latest="$f"
        fi
    done
    if [ -n "$latest" ]; then
        basename "$latest"
    fi
}

release_cpu_baseline_to_march() {
    case "$1" in
        "")
            printf '%s\n' "-march=native"
            ;;
        x86-64|x86-64-v3)
            printf '%s\n' "-march=$1"
            ;;
        *)
            echo -e "${RED}ERROR: SEEN_RELEASE_CPU_BASELINE must be x86-64 or x86-64-v3.${NC}" >&2
            exit 1
            ;;
    esac
}

memory_guard_enabled() {
    [ "${SEEN_DISABLE_MEMORY_GUARD:-0}" != "1" ] &&
        [ -x "$MEMORY_GUARD_SCRIPT" ] &&
        { [ -n "${MEMORY_GUARD_RSS_KB:-}" ] || [ -n "${MEMORY_GUARD_RESERVE_KB:-}" ]; }
}

user_memory_scope_available() {
    command -v systemd-run >/dev/null 2>&1 || return 1
    command -v systemctl >/dev/null 2>&1 || return 1
    systemctl --user show-environment >/dev/null 2>&1
}

verify_rebuild_kernel_scope() {
    REBUILD_SCOPE_EVIDENCE_FILE="$REBUILD_WORK_ROOT/rebuild-containment-preflight.metrics"
    "$MEMORY_GUARD_SCRIPT" \
        --label "rebuild containment preflight" \
        --rss-limit-kb "$MEMORY_GUARD_RSS_KB" \
        --available-reserve-kb "$MEMORY_GUARD_RESERVE_KB" \
        --vmem-limit-kb "$OPT_VMEM_KB" \
        --timeout-secs 5 \
        --tasks-max "$MEMORY_GUARD_TASKS_MAX" \
        --cgroup-stop-kb "$MEMORY_GUARD_CGROUP_STOP_KB" \
        --kill-only \
        --metrics-file "$REBUILD_SCOPE_EVIDENCE_FILE" \
        -- sh -c 'sleep 0.05'
}

enter_rebuild_kernel_scope() {
    local aggregate_metrics="$REBUILD_WORK_ROOT/safe_rebuild_${REBUILD_TIER}.aggregate.metrics"
    local successful_metrics="$REBUILD_ARTIFACT_SCOPE/last-success-${REBUILD_TIER}.aggregate.metrics"
    local supervisor_tmp="$REBUILD_WORK_ROOT/.aggregate-supervisor"
    if [ -L "$supervisor_tmp" ] || { [ -e "$supervisor_tmp" ] && [ ! -d "$supervisor_tmp" ]; }; then
        echo -e "${RED}ERROR: unsafe aggregate supervisor directory: $supervisor_tmp${NC}" >&2
        exit 1
    fi
    mkdir -p -- "$supervisor_tmp" || exit 1
    echo -e "${YELLOW}Entering verified aggregate rebuild cgroup before any build starts.${NC}"
    exec env \
        TMPDIR="$supervisor_tmp" \
        SEEN_MEMORY_GUARD_REMOVE_EMPTY_TMPDIR=1 \
        "$MEMORY_GUARD_SCRIPT" \
        --label "safe rebuild $REBUILD_TIER aggregate" \
        --rss-limit-kb "$MEMORY_GUARD_RSS_KB" \
        --available-reserve-kb "$MEMORY_GUARD_RESERVE_KB" \
        --vmem-limit-kb "$MAIN_COMPILER_VMEM_KB" \
        --tasks-max "$MEMORY_GUARD_TASKS_MAX" \
        --cgroup-stop-kb "$MEMORY_GUARD_CGROUP_STOP_KB" \
        --kill-only \
        --metrics-file "$aggregate_metrics" \
        --success-metrics-file "$successful_metrics" \
        -- env \
            SEEN_REBUILD_AGGREGATE_SCOPE_ACTIVE=1 \
            "$SCRIPT_DIR/safe_rebuild.sh" "${SAFE_REBUILD_ORIGINAL_ARGS[@]}"
}

prepare_bounded_wine_prefix_template() {
    local template_root="$REBUILD_WORK_ROOT/wine-prefix-template"
    local template_prefix="$template_root/prefix"
    local wine_cpu=""
    local cpu_key cpu_value

    [ "$REBUILD_TIER" = "verify" ] || return 0
    command -v wine >/dev/null 2>&1 || return 0
    command -v wineboot >/dev/null 2>&1 || return 0
    command -v wineserver >/dev/null 2>&1 || return 0
    command -v taskset >/dev/null 2>&1 || {
        echo -e "${RED}ERROR: Wine prefix preparation requires taskset.${NC}" >&2
        return 126
    }
    while read -r cpu_key cpu_value; do
        if [ "$cpu_key" = "Cpus_allowed_list:" ]; then
            wine_cpu="${cpu_value%%[-,]*}"
            break
        fi
    done < /proc/self/status
    case "$wine_cpu" in
        ''|*[!0-9]*)
            echo -e "${RED}ERROR: could not select an allowed CPU for Wine prefix preparation.${NC}" >&2
            return 126
            ;;
    esac
    if [ -e "$template_root" ] || [ -L "$template_root" ]; then
        echo -e "${RED}ERROR: refusing an existing Wine prefix template path.${NC}" >&2
        return 126
    fi
    mkdir -p -- "$template_root/home" "$template_root/cache" \
        "$template_root/config" "$template_root/data" || return 1
    if ! "$HARD_MEMORY_SCOPE_WRAPPER" \
        --label "Wine prefix preparation" -- \
        bash -c '
            set -euo pipefail
            template_root=$1
            wine_cpu=$2
            cleanup() {
                status=$?
                env WINEPREFIX="$template_root/prefix" wineserver -k \
                    >/dev/null 2>&1 || true
                env WINEPREFIX="$template_root/prefix" wineserver -w \
                    >/dev/null 2>&1 || true
                exit "$status"
            }
            trap cleanup EXIT
            env \
                HOME="$template_root/home" \
                XDG_CACHE_HOME="$template_root/cache" \
                XDG_CONFIG_HOME="$template_root/config" \
                XDG_DATA_HOME="$template_root/data" \
                WINEARCH=win64 \
                WINEPREFIX="$template_root/prefix" \
                WINEDEBUG=-all \
                WINEDLLOVERRIDES="explorer.exe,services.exe,winemenubuilder.exe=d" \
                WINE_CPU_TOPOLOGY=1:1 \
                taskset -c "$wine_cpu" wineboot --init
            env WINEPREFIX="$template_root/prefix" wineserver -w
        ' bash "$template_root" "$wine_cpu"; then

        echo -e "${RED}ERROR: bounded Wine prefix preparation failed.${NC}" >&2
        return 1
    fi
    [ -d "$template_prefix" ] && [ ! -L "$template_prefix" ] || {
        echo -e "${RED}ERROR: bounded Wine prefix preparation produced no safe prefix.${NC}" >&2
        return 1
    }
    export SEEN_WINE_PREFIX_TEMPLATE="$template_prefix"
}

run_guarded_command() {
    local label=$1
    local timeout_secs=$2
    local vmem_kb=$3
    shift 3

    if ! memory_guard_enabled; then
        echo -e "${RED}ERROR: refusing unguarded rebuild command: $label.${NC}" >&2
        return 126
    fi

    # The aggregate scope has already been read back against its transient
    # cgroup. Do not consume additional pids on a redundant nested observer;
    # retain the command's VMEM and wall-clock limits while the aggregate
    # cgroup remains the authoritative memory/swap/task boundary.
    if [ "${SEEN_REBUILD_AGGREGATE_SCOPE_VERIFIED:-0}" = "1" ]; then
        local scoped_status=0
        if [ -n "$timeout_secs" ] && [ "$timeout_secs" != "0" ]; then
            (
                [ -z "$vmem_kb" ] || ulimit -S -v "$vmem_kb"
                exec timeout -k 1 "$timeout_secs" "$@"
            ) || scoped_status=$?
        else
            (
                [ -z "$vmem_kb" ] || ulimit -S -v "$vmem_kb"
                exec "$@"
            ) || scoped_status=$?
        fi
        case "$scoped_status" in
            124|125|126|137|143)
                echo -e "${RED}FATAL: $label hit containment status $scoped_status; aborting the rebuild without fallback or retry.${NC}" >&2
                record_rebuild_fatal_status "$scoped_status" || true
                kill -TERM "$$" 2>/dev/null || true
                ;;
        esac
        return "$scoped_status"
    fi

    local guard_cmd=("$MEMORY_GUARD_SCRIPT" --label "$label")
    if [ -n "${MEMORY_GUARD_RSS_KB:-}" ]; then
        guard_cmd+=(--rss-limit-kb "$MEMORY_GUARD_RSS_KB")
    fi
    if [ -n "${MEMORY_GUARD_RESERVE_KB:-}" ]; then
        guard_cmd+=(--available-reserve-kb "$MEMORY_GUARD_RESERVE_KB")
    fi
    if [ -n "$vmem_kb" ]; then
        guard_cmd+=(--vmem-limit-kb "$vmem_kb")
    fi
    if [ -n "$timeout_secs" ] && [ "$timeout_secs" != "0" ]; then
        guard_cmd+=(--timeout-secs "$timeout_secs")
    fi
    if [ -n "${MEMORY_GUARD_TASKS_MAX:-}" ]; then
        guard_cmd+=(--tasks-max "$MEMORY_GUARD_TASKS_MAX")
    fi
    if [ -n "${MEMORY_GUARD_CGROUP_STOP_KB:-}" ]; then
        guard_cmd+=(--cgroup-stop-kb "$MEMORY_GUARD_CGROUP_STOP_KB")
    fi
    if [ "${SEEN_MEMORY_GUARD_KILL_ONLY:-0}" = "1" ]; then
        guard_cmd+=(--kill-only)
    fi
    if [ -n "${SEEN_MEMORY_GUARD_METRICS_FILE:-}" ]; then
        guard_cmd+=(--metrics-file "$SEEN_MEMORY_GUARD_METRICS_FILE")
    fi
    guard_cmd+=(-- "$@")
    local guarded_status=0
    "${guard_cmd[@]}" || guarded_status=$?
    case "$guarded_status" in
        124|125|126|137|143)
            echo -e "${RED}FATAL: $label hit containment status $guarded_status; aborting the rebuild without fallback or retry.${NC}" >&2
            record_rebuild_fatal_status "$guarded_status" || true
            kill -TERM "$$" 2>/dev/null || true
            ;;
    esac
    return "$guarded_status"
}

run_guarded_command_to_log() {
    local label=$1
    local timeout_secs=$2
    local vmem_kb=$3
    local log_file=$4
    shift 4

    local guard_log="${log_file%.log}.guard.log"
    local guard_metrics="${log_file%.log}.guard.metrics"
    local trace_start=""
    local guard_state=""
    local guard_status=""
    local resource_stop=0
    if declare -F seen_build_trace_step_start >/dev/null 2>&1; then
        trace_start=$(seen_build_trace_step_start "$label")
    fi
    : > "$log_file"
    : > "$guard_log"
    rm -f "$guard_metrics"

    local status=0
    if [ "${SEEN_REBUILD_AGGREGATE_SCOPE_VERIFIED:-0}" = "1" ]; then
        # The aggregate cgroup is already the authenticated observer. Redirect
        # the bounded command directly so a logger shell does not consume one
        # of the deliberately scarce task slots for the command's lifetime.
        SEEN_MEMORY_GUARD_METRICS_FILE="$guard_metrics" \
        run_guarded_command "$label" "$timeout_secs" "$vmem_kb" \
            "$@" > "$log_file" 2>&1 || status=$?
    else
        SEEN_MEMORY_GUARD_METRICS_FILE="$guard_metrics" \
        run_guarded_command "$label" "$timeout_secs" "$vmem_kb" \
            bash -c '
                log_file=$1
                shift
                exec "$@" > "$log_file" 2>&1
            ' bash "$log_file" "$@" > "$guard_log" 2>&1 || status=$?
    fi

    if [ -s "$guard_log" ]; then
        {
            echo ""
            echo "[memory guard]"
            cat "$guard_log"
        } >> "$log_file" 2>/dev/null || true
    fi

    if [ -f "$guard_metrics" ]; then
        guard_state=$(awk -F= '/^state=/ {print $2; exit}' "$guard_metrics" 2>/dev/null || true)
        guard_status=$(awk -F= '/^command_status=/ {print $2; exit}' "$guard_metrics" 2>/dev/null || true)
    fi
    case "$status" in
        124|125|126|137|143) resource_stop=1 ;;
    esac
    case "$guard_state" in
        rss_limit|cgroup_limit|reserve_limit|tasks_limit|timeout|\
        detached_descendants|startup_failure)
            resource_stop=1
            ;;
    esac
    # A compiler or LLVM subprocess can catch an allocation failure and exit
    # with an ordinary status (commonly 1), leaving the guard metrics in the
    # otherwise-successful "complete" state. Treat only explicit allocation
    # diagnostics on a nonzero command as a containment failure so candidate
    # and recovery loops can never retry after an OOM-like stop.
    if [ "$status" -ne 0 ] && grep -Eiq \
        '(^|[^[:alnum:]_])(resource stop:|out of memory|cannot allocate memory|could not allocate memory|memory allocation (failed|failure)|allocation failure|std::bad_alloc|bad_alloc|resource temporarily unavailable|cannot fork|can.t fork|fork: retry|fork (failed|failure)|pthread_create([^[:alnum:]_].*)?(failed|failure)|failed to create (a )?thread|can.t create (a )?thread|cannot create (a )?thread|thread creation (failed|failure))([^[:alnum:]_]|$)' \
        "$log_file" 2>/dev/null; then

        resource_stop=1
        guard_state="resource_diagnostic"
    fi

    if declare -F seen_build_trace_step_end >/dev/null 2>&1; then
        local trace_detail="log=$log_file"
        local peak_rss_kb peak_cgroup_kb
        if [ -f "$guard_metrics" ]; then
            peak_rss_kb=$(awk -F= '/^peak_rss_kb=/ {print $2; exit}' "$guard_metrics" 2>/dev/null || true)
            peak_cgroup_kb=$(awk -F= '/^peak_cgroup_kb=/ {print $2; exit}' "$guard_metrics" 2>/dev/null || true)
            if [ -n "$peak_rss_kb" ]; then
                trace_detail="$trace_detail peak_rss_kb=$peak_rss_kb"
            fi
            if [ -n "$peak_cgroup_kb" ]; then
                trace_detail="$trace_detail peak_cgroup_kb=$peak_cgroup_kb"
            fi
            if [ -n "$guard_state" ]; then
                trace_detail="$trace_detail guard_state=$guard_state"
            fi
            if [ -n "$guard_status" ]; then
                trace_detail="$trace_detail guard_status=$guard_status"
            fi
        fi
        if [ "$status" -eq 0 ]; then
            seen_build_trace_step_end "$label" "$trace_start" "ok" "$trace_detail"
        else
            seen_build_trace_step_end "$label" "$trace_start" "failed:$status" "$trace_detail"
        fi
    fi
    if [ "$resource_stop" -eq 1 ]; then
        [ "$status" -ne 0 ] || status=125
        echo -e "${RED}FATAL: $label hit a resource/containment stop (status=$status, guard_state=${guard_state:-unknown}); aborting the rebuild without fallback or retry.${NC}" >&2
        record_rebuild_fatal_status "$status" || true
        kill -TERM "$$" 2>/dev/null || true
    fi
    return "$status"
}

log_failure_signal_pattern() {
    printf '%s\n' 'Fatal Lexer Error|Fatal Parser Error|IR VERIFY|llvm-as:|/usr/bin/opt:|clang: error|ld\.lld: error|LLVM ERROR|Error: optimization failed|Segmentation fault|core dumped|Traceback \(most recent call last\)|(^|[[:space:]])Error:'
}

start_log_failure_watcher() {
    # Failure classification runs once after the authenticated guard has
    # synchronously stopped and drained the command. A polling watcher adds a
    # shell, grep, and sleep to the 16-task aggregate budget without being able
    # to stop the guarded cgroup safely.
    printf '%s\n' ""
}

run_guarded_command_to_log_with_failure_watch() {
    local label=$1
    local timeout_secs=$2
    local vmem_kb=$3
    local log_file=$4
    shift 4

    run_guarded_command_to_log "$label" "$timeout_secs" "$vmem_kb" \
        "$log_file" "$@"
}

build_fork_serializer() {
    local serializer_tmp=""
    local attestation_tmp=""
    local serializer_sha=""
    local source_sha=""
    local selftest_source_sha=""
    local body_sha=""
    local scope_unit="${SEEN_MEMORY_GUARD_SCOPE_UNIT:-}"

    if [ "$HOST_OS" != "Linux" ]; then
        echo -e "${RED}ERROR: rebuild candidates require a verified fork serializer, which is currently available only on Linux.${NC}" >&2
        return 1
    fi
    if [ ! -f "$FORK_SERIALIZER_SOURCE" ] ||
        [ ! -f "$FORK_SERIALIZER_SELFTEST_SOURCE" ]; then

        echo -e "${RED}ERROR: required fork serializer source/self-test is missing.${NC}" >&2
        return 1
    fi
    if ! command -v clang >/dev/null 2>&1; then
        echo -e "${RED}ERROR: clang unavailable; cannot build the required fork serializer.${NC}" >&2
        return 1
    fi
    if [ ! -x "$FORK_SERIALIZER_VERIFY_SCRIPT" ] ||
        [ ! -x "$BUILDER_APPLICABILITY_SCRIPT" ]; then

        echo -e "${RED}ERROR: fork serializer verification helpers are missing.${NC}" >&2
        return 1
    fi
    case "$scope_unit" in
        seen-memory-guard-*.scope) ;;
        *)
            echo -e "${RED}ERROR: verified scope unit is unavailable for serializer attestation.${NC}" >&2
            return 1
            ;;
    esac

    case "$REBUILD_WORK_ROOT" in
        "$REBUILD_ARTIFACT_SCOPE"/run.*) ;;
        *)
            echo -e "${RED}ERROR: fork serializer output root is not the verified rebuild artifact directory.${NC}" >&2
            return 1
            ;;
    esac
    FORK_SERIALIZER_SO="$REBUILD_WORK_ROOT/seen-fork-serializer.so"
    FORK_SERIALIZER_ATTESTATION="$REBUILD_WORK_ROOT/seen-fork-serializer.attestation"
    if [ -L "$FORK_SERIALIZER_SO" ]; then
        echo -e "${RED}ERROR: refusing symbolic-link fork serializer output.${NC}" >&2
        return 1
    fi
    serializer_tmp=$(mktemp "$REBUILD_WORK_ROOT/.seen-fork-serializer.XXXXXX.so") || return 1
    if run_guarded_command "fork serializer build" 60 "${OPT_VMEM_KB:-}" \
        clang -shared -fPIC -O2 "$FORK_SERIALIZER_SOURCE" -o "$serializer_tmp" -ldl -pthread &&
        [ -f "$serializer_tmp" ] && [ ! -L "$serializer_tmp" ] &&
        mv -f -- "$serializer_tmp" "$FORK_SERIALIZER_SO"; then

        local selftest_binary="$REBUILD_WORK_ROOT/seen-fork-serializer-selftest"
        local selftest_state="$REBUILD_WORK_ROOT/seen-fork-serializer-selftest.state"
        local descendant_selftest="$REBUILD_WORK_ROOT/seen-fork-serializer-descendant-selftest"
        local cachetest_serializer="$REBUILD_WORK_ROOT/seen-fork-serializer-cachetest.so"
        if [ -L "$selftest_binary" ] || [ -L "$selftest_state" ] ||
            [ -L "$descendant_selftest" ] ||
            [ -L "$cachetest_serializer" ]; then

            echo -e "${RED}ERROR: refusing unsafe fork serializer self-test paths.${NC}" >&2
            return 1
        fi
        {
            printf '%s\n' '#!/usr/bin/env bash'
            printf '%s\n' 'set -euo pipefail'
            printf '%s\n' 'program=$1'
            printf '%s\n' 'state=$2'
            printf '%s\n' 'for _ in 1 2 3 4 5 6 7 8; do "$program" --child "$state" & done'
            printf '%s\n' 'wait'
        } > "$descendant_selftest"
        chmod 700 "$descendant_selftest"
        if ! run_guarded_command "fork serializer self-test build" 60 "${OPT_VMEM_KB:-}" \
            clang -O2 -pthread "$FORK_SERIALIZER_SELFTEST_SOURCE" \
                -o "$selftest_binary"; then

            echo -e "${RED}ERROR: failed to build the capped fork serializer self-test.${NC}" >&2
            return 1
        fi
        if ! run_guarded_command "fork serializer cache-limit test build" 60 "${OPT_VMEM_KB:-}" \
            clang -shared -fPIC -O2 -DSERIALIZER_STATUS_CAPACITY=2 \
                "$FORK_SERIALIZER_SOURCE" -o "$cachetest_serializer" \
                -ldl -pthread; then

            echo -e "${RED}ERROR: failed to build the capped serializer cache-limit test shim.${NC}" >&2
            return 1
        fi
        if ! run_guarded_command "fork serializer cache-limit self-test" 20 "${OPT_VMEM_KB:-}" \
            env -u SEEN_FORK_SERIALIZER_ROOT_PID \
                LD_PRELOAD="$cachetest_serializer" \
                SEEN_FORK_SERIALIZER_TARGET="$selftest_binary" \
                "$selftest_binary" --cache-overflow; then

            echo -e "${RED}ERROR: fork serializer cache limit did not fail closed.${NC}" >&2
            return 1
        fi
        if ! run_guarded_command "fork serializer dynamic self-test" 30 "${OPT_VMEM_KB:-}" \
            env -u SEEN_FORK_SERIALIZER_ROOT_PID \
                LD_PRELOAD="$FORK_SERIALIZER_SO" \
                SEEN_FORK_SERIALIZER_TARGET="$selftest_binary" \
                "$selftest_binary" "$selftest_state"; then

            echo -e "${RED}ERROR: fork serializer dynamic self-test failed; compiler candidates remain disabled.${NC}" >&2
            return 1
        fi
        if ! run_guarded_command "fork serializer descendant self-test" 30 "${OPT_VMEM_KB:-}" \
            env -u SEEN_FORK_SERIALIZER_ROOT_PID \
                LD_PRELOAD="$FORK_SERIALIZER_SO" \
                SEEN_FORK_SERIALIZER_TARGET="$selftest_binary" \
                SEEN_FORK_SERIALIZER_DESCENDANT_SCRIPT="$descendant_selftest" \
                "$selftest_binary" --descendant-script "$descendant_selftest" \
                "$selftest_state.descendant"; then

            echo -e "${RED}ERROR: fork serializer descendant self-test failed; compiler candidates remain disabled.${NC}" >&2
            return 1
        fi
        if run_guarded_command "fork serializer target rejection self-test" 10 "${OPT_VMEM_KB:-}" \
            env -u SEEN_FORK_SERIALIZER_ROOT_PID \
                LD_PRELOAD="$FORK_SERIALIZER_SO" \
                SEEN_FORK_SERIALIZER_TARGET="$FORK_SERIALIZER_SOURCE" \
                "$selftest_binary" "$selftest_state.target-rejection"; then

            echo -e "${RED}ERROR: fork serializer accepted a mismatched root target.${NC}" >&2
            return 1
        fi
        serializer_sha=$(sha256sum "$FORK_SERIALIZER_SO" | awk '{print $1}') || return 1
        source_sha=$(sha256sum "$FORK_SERIALIZER_SOURCE" | awk '{print $1}') || return 1
        selftest_source_sha=$(sha256sum "$FORK_SERIALIZER_SELFTEST_SOURCE" | awk '{print $1}') || return 1
        attestation_tmp=$(mktemp "$REBUILD_WORK_ROOT/.seen-fork-serializer-attestation.XXXXXX") || return 1
        {
            printf 'version=seen-fork-serializer-attestation-v2\n'
            printf 'serializer_sha256=%s\n' "$serializer_sha"
            printf 'source_sha256=%s\n' "$source_sha"
            printf 'selftest_source_sha256=%s\n' "$selftest_source_sha"
            printf 'dynamic_selftest=passed\n'
            printf 'scope_unit=%s\n' "$scope_unit"
            printf 'verified_memory_max_bytes=%s\n' "$verified_memory_max"
            printf 'verified_memory_swap_max_bytes=%s\n' "$verified_memory_swap_max"
            printf 'verified_pids_max=%s\n' "$verified_pids_max"
        } > "$attestation_tmp"
        body_sha=$(sha256sum "$attestation_tmp" | awk '{print $1}') || return 1
        printf 'body_sha256=%s\n' "$body_sha" >> "$attestation_tmp"
        mv -f -- "$attestation_tmp" "$FORK_SERIALIZER_ATTESTATION"
        export SEEN_FORK_SERIALIZER_SO="$FORK_SERIALIZER_SO"
        export SEEN_FORK_SERIALIZER_ATTESTATION="$FORK_SERIALIZER_ATTESTATION"
        if ! bash "$FORK_SERIALIZER_VERIFY_SCRIPT" \
            "$FORK_SERIALIZER_SO" "$FORK_SERIALIZER_ATTESTATION" \
            "$REBUILD_WORK_ROOT" "$scope_unit" >/dev/null; then

            echo -e "${RED}ERROR: fork serializer attestation verification failed.${NC}" >&2
            return 1
        fi
        echo -e "${YELLOW}Verified-scope direct compiler-child serialization enabled; the hard cgroup remains authoritative for descendant tools.${NC}"
    else
        echo -e "${RED}ERROR: failed to build the required fork serializer.${NC}" >&2
        rm -f -- "$serializer_tmp"
        [ -z "$attestation_tmp" ] || rm -f -- "$attestation_tmp"
        FORK_SERIALIZER_SO=""
        FORK_SERIALIZER_ATTESTATION=""
        return 1
    fi
}

compiler_serializer_applicable() {
    local compiler_path=$1

    [ -n "$FORK_SERIALIZER_SO" ] && [ -n "$FORK_SERIALIZER_ATTESTATION" ] ||
        return 1
    bash "$FORK_SERIALIZER_VERIFY_SCRIPT" \
        "$FORK_SERIALIZER_SO" "$FORK_SERIALIZER_ATTESTATION" \
        "$REBUILD_WORK_ROOT" "${SEEN_MEMORY_GUARD_SCOPE_UNIT:-}" >/dev/null ||
        return 1
    SEEN_MEMORY_GUARD_IN_SCOPE=1 \
        bash "$BUILDER_APPLICABILITY_SCRIPT" \
            "$compiler_path" "$FORK_SERIALIZER_SO" >/dev/null
}

prepare_bounded_toolchain() {
    [ -x "$BOUNDED_TOOLCHAIN_PREPARE_SCRIPT" ] || {
        echo -e "${RED}ERROR: bounded-toolchain helper is missing.${NC}" >&2
        return 1
    }
    BOUNDED_TOOLCHAIN_DIR=$(bash "$BOUNDED_TOOLCHAIN_PREPARE_SCRIPT" \
        "$REBUILD_WORK_ROOT") || return 1
    case "$BOUNDED_TOOLCHAIN_DIR" in
        "$REBUILD_WORK_ROOT"/bounded-toolchain) ;;
        *)
            echo -e "${RED}ERROR: bounded-toolchain helper returned an unsafe path.${NC}" >&2
            return 1
            ;;
    esac
    [ -d "$BOUNDED_TOOLCHAIN_DIR" ] && [ ! -L "$BOUNDED_TOOLCHAIN_DIR" ] ||
        return 1
    PATH="$BOUNDED_TOOLCHAIN_DIR:$PATH"
    export PATH SEEN_BOUNDED_TOOLCHAIN_DIR="$BOUNDED_TOOLCHAIN_DIR"
}

# Monitor a compilation step in background, printing live progress.
# Usage: monitor_compilation <PID> <stage_label>
# Watches /tmp/seen_module_*.ll files to track per-module progress.
monitor_compilation() {
    local compile_pid=$1
    local label=$2
    local start_time=$SECONDS
    local total_modules=0
    local last_status=""

    while kill -0 "$compile_pid" 2>/dev/null; do
        local elapsed=$((SECONDS - start_time))
        local elapsed_fmt=$(format_time $elapsed)

        # Count plain generated IR separately from fixed/optimized IR so progress
        # doesn't look like the compiler is discovering new source modules forever.
        local ll_count=$(count_plain_module_lls /tmp)
        local opt_ll_count=$(count_module_opt_lls /tmp)

        # Count .o files (modules fully compiled)
        local obj_count=$(count_module_objects /tmp)
        local latest_ll=$(latest_plain_module_ll /tmp)

        # Check for module 5 (the big one) -- if its .ll exists
        local mod5_status=""
        if [ -f /tmp/seen_module_5.ll ]; then
            local mod5_size=$(stat -c%s /tmp/seen_module_5.ll 2>/dev/null || stat -f%z /tmp/seen_module_5.ll 2>/dev/null || echo 0)
            mod5_status="mod5.ll=$(format_bytes $mod5_size)"
        else
            # Check if a fork child is working on it (large RSS process)
            local fork_pids=$(pgrep -P "$compile_pid" 2>/dev/null || true)
            if [ -n "$fork_pids" ]; then
                for fpid in $fork_pids; do
                    local frss=$(ps -o rss= -p "$fpid" 2>/dev/null | tr -d ' ')
                    if [ -n "$frss" ] && [ "$frss" -gt 500000 ]; then
                        mod5_status="mod5: IR gen ($(format_bytes $((frss * 1024))) RSS)"
                        break
                    fi
                done
            fi
            if [ -z "$mod5_status" ]; then
                mod5_status="mod5: waiting"
            fi
        fi

        # Check if we're in opt/link phase (parallel opt script exists and running)
        local phase="IR gen"
        if pgrep -f "seen_parallel_opt.sh" > /dev/null 2>&1; then
            phase="opt"
        fi
        if pgrep -f "clang.*flto.*seen_module" > /dev/null 2>&1 || pgrep -f "ld.lld.*seen_module" > /dev/null 2>&1; then
            phase="link"
        fi

        local ll_status="${BOLD}${ll_count} raw.ll${NC}"
        if [ "$opt_ll_count" -gt 0 ]; then
            ll_status="${ll_status}/${BOLD}${opt_ll_count} opt.ll${NC}"
        fi

        local latest_status=""
        if [ -n "$latest_ll" ]; then
            latest_status=" latest=${latest_ll}"
        fi

        # Build status line
        local status="${CYAN}[$label]${NC} ${elapsed_fmt}  ${ll_status} | ${BOLD}${obj_count} .o${NC}  phase:${phase}  ${DIM}${mod5_status}${latest_status}${NC}"

        # Only reprint if status changed (avoid flicker)
        if [ "$status" != "$last_status" ]; then
            printf "\r\033[K${status}"
            last_status="$status"
        fi

        sleep 5
    done

    # Final status
    local elapsed=$((SECONDS - start_time))
    local elapsed_fmt=$(format_time $elapsed)
    local ll_count=$(count_plain_module_lls /tmp)
    local opt_ll_count=$(count_module_opt_lls /tmp)
    local obj_count=$(count_module_objects /tmp)
    printf "\r\033[K${CYAN}[$label]${NC} ${GREEN}done${NC} in ${elapsed_fmt}  ${ll_count} raw.ll | ${opt_ll_count} opt.ll | ${obj_count} .o\n"
}

# Run a compilation step with live progress monitoring.
# Usage: run_with_progress <label> <command...>
# Returns the exit code of the compilation command.
run_with_progress() {
    local label=$1
    shift
    local logfile=$1
    shift

    # Keep the guarded command in the foreground. The aggregate owner supplies
    # cgroup-native progress/accounting without extra polling shells.
    : > "$logfile"
    local exit_code=0
    run_guarded_command_to_log "$label" 0 "${MAIN_COMPILER_VMEM_KB:-}" \
        "$logfile" "$@" || exit_code=$?
    return "$exit_code"
}

# Snapshot watcher: periodically copies Stage2 module artifacts to a safe
# directory so they survive frozen compiler cleanup (which deletes
# /tmp/seen_module_*). Plain .ll files are used for recovery; opt logs/statuses
# are kept so the first concrete failure can be inspected without another
# rebuild.
# Usage: start_ll_snapshot_watcher <compiler_pid> <snapshot_dir>
start_ll_snapshot_watcher() {
    local watch_pid=$1
    local snapshot_dir=$2
    mkdir -p "$snapshot_dir"
    while kill -0 "$watch_pid" 2>/dev/null; do
        for f in /tmp/seen_module_*.ll /tmp/seen_module_*.opt.ll \
            /tmp/seen_module_*.opt.log /tmp/seen_module_*.opt.status; do
            [ -f "$f" ] || continue
            [[ "$f" == *.polly.ll ]] && continue
            local bn=$(basename "$f")
            if [ ! -f "$snapshot_dir/$bn" ] || [ "$f" -nt "$snapshot_dir/$bn" ]; then
                cp "$f" "$snapshot_dir/$bn" 2>/dev/null || true
            fi
        done
        sleep 1
    done
    # Final sweep after compiler exits
    for f in /tmp/seen_module_*.ll /tmp/seen_module_*.opt.ll \
        /tmp/seen_module_*.opt.log /tmp/seen_module_*.opt.status; do
        [ -f "$f" ] || continue
        [[ "$f" == *.polly.ll ]] && continue
        cp "$f" "$snapshot_dir/$(basename "$f")" 2>/dev/null || true
    done
}

finish_ll_snapshot_watcher() {
    local watcher_pid=$1
    local waited=0
    while kill -0 "$watcher_pid" 2>/dev/null; do
        if [ "$waited" -ge 10 ]; then
            kill "$watcher_pid" 2>/dev/null || true
            break
        fi
        sleep 1
        waited=$((waited+1))
    done
    wait "$watcher_pid" 2>/dev/null || true
}

kill_scope_matching_processes() {
    local pattern=$1
    local current_cgroup=""
    local candidate_cgroup=""
    local matched_pids=()

    current_cgroup=$(awk -F: '$1 == "0" { print $3; exit }' /proc/self/cgroup 2>/dev/null || true)
    if [ -z "$current_cgroup" ] || [ "$current_cgroup" = "/" ]; then
        echo -e "${RED}ERROR: refusing process cleanup without an isolated rebuild cgroup.${NC}" >&2
        return 1
    fi
    while IFS= read -r pid; do
        [ -n "$pid" ] || continue
        candidate_cgroup=$(awk -F: '$1 == "0" { print $3; exit }' "/proc/$pid/cgroup" 2>/dev/null || true)
        if [ "$candidate_cgroup" = "$current_cgroup" ]; then
            matched_pids+=("$pid")
        fi
    done < <(ps -eo pid=,args= | awk -v pat="$pattern" -v self="$$" '$0 ~ pat && $1 != self { print $1 }')
    if [ "${#matched_pids[@]}" -gt 0 ]; then
        kill -KILL "${matched_pids[@]}" 2>/dev/null || true
    fi
}

# Kill only orphaned fork children in this rebuild's verified transient cgroup.
# SIGTERM triggers legacy cleanup handlers which delete recovery files.
kill_frozen_orphans() {
    kill_scope_matching_processes "$(basename "$FROZEN")"
    kill_scope_matching_processes "seen_parallel_opt"
    kill_scope_matching_processes "opt.*seen_module"
    kill_scope_matching_processes "clang.*seen_module"
    kill_scope_matching_processes "ld.lld.*seen_module"
    sleep 2
}

copy_bootstrap_seen_tree() {
    local src_dir=$1
    local dst_dir=$2
    local canonical_src link target file src_file dst_file
    canonical_src=$(cd "$src_dir" && pwd -P) || return 1

    while IFS= read -r -d '' link; do
        target=$(readlink -f "$link" 2>/dev/null || true)
        case "$target" in
            "$canonical_src"/*) ;;
            *)
                echo -e "${RED}ERROR: bootstrap source link escapes its tree: $link.${NC}" >&2
                return 1
                ;;
        esac
        if [ ! -e "$target" ]; then
            echo -e "${RED}ERROR: bootstrap source link is dangling: $link.${NC}" >&2
            return 1
        fi
    done < <(find "$src_dir" -type l -print0)

    mkdir -p "$dst_dir"
    cp -aL "$src_dir/." "$dst_dir/" || return 1
    if find "$dst_dir" -type l -print -quit | grep -q .; then
        echo -e "${RED}ERROR: bootstrap source view retained a symbolic link.${NC}" >&2
        return 1
    fi
    while IFS= read -r -d '' file; do
        src_file="$src_dir/$file"
        dst_file="$dst_dir/$file"
        if ! cmp -s "$src_file" "$dst_file"; then
            echo -e "${RED}ERROR: bootstrap source view changed $src_file.${NC}" >&2
            return 1
        fi
    done < <(cd "$src_dir" && find -L . -type f -print0)
}

prepare_bootstrap_source_overlay() {
    if [ "${SEEN_DISABLE_BOOTSTRAP_SOURCE_OVERLAY:-0}" = "1" ]; then
        BOOTSTRAP_SOURCE_ROOT="$REPO_ROOT"
        return 0
    fi

    BOOTSTRAP_SOURCE_ROOT=$(mktemp -d /tmp/seen_bootstrap_source.XXXXXX)
    for entry in "$REPO_ROOT"/*; do
        local base
        base=$(basename "$entry")
        case "$base" in
            bootstrap|compiler_seen|seen_std|releases)
                ;;
            *)
                ln -s "$entry" "$BOOTSTRAP_SOURCE_ROOT/$base"
                ;;
        esac
    done

    if [ -d "$REPO_ROOT/bootstrap" ]; then
        mkdir -p "$BOOTSTRAP_SOURCE_ROOT/bootstrap"
        for entry in "$REPO_ROOT/bootstrap"/*; do
            [ -e "$entry" ] || continue
            local base
            base=$(basename "$entry")
            case "$base" in
                stage1_frozen*|seen_frozen*)
                    cp -pL "$entry" "$BOOTSTRAP_SOURCE_ROOT/bootstrap/$base"
                    chmod +x "$BOOTSTRAP_SOURCE_ROOT/bootstrap/$base" 2>/dev/null || true
                    ;;
                *)
                    ln -s "$entry" "$BOOTSTRAP_SOURCE_ROOT/bootstrap/$base"
                    ;;
            esac
        done
    fi

    mkdir -p "$BOOTSTRAP_SOURCE_ROOT/compiler_seen" "$BOOTSTRAP_SOURCE_ROOT/seen_std"
    for entry in "$REPO_ROOT/compiler_seen"/*; do
        local base
        base=$(basename "$entry")
        if [ "$base" = "src" ]; then
            copy_bootstrap_seen_tree "$entry" "$BOOTSTRAP_SOURCE_ROOT/compiler_seen/src"
        elif [ "$base" = "target" ]; then
            mkdir -p "$BOOTSTRAP_SOURCE_ROOT/compiler_seen/target"
            for bin in "$entry"/seen "$entry"/seen_frozen* "$entry"/stage1_frozen* "$entry"/seen_native_snapshot; do
                [ -f "$bin" ] || continue
                cp -pL "$bin" "$BOOTSTRAP_SOURCE_ROOT/compiler_seen/target/$(basename "$bin")"
                chmod +x "$BOOTSTRAP_SOURCE_ROOT/compiler_seen/target/$(basename "$bin")" 2>/dev/null || true
            done
        else
            ln -s "$entry" "$BOOTSTRAP_SOURCE_ROOT/compiler_seen/$base"
        fi
    done
    # seen_std is a local package dependency. Keep its overlay symlink-free so
    # the package client's local-source hardening sees the same regular-file
    # layout that a published package archive contains.
    copy_bootstrap_seen_tree "$REPO_ROOT/seen_std" "$BOOTSTRAP_SOURCE_ROOT/seen_std"

    # A frozen compiler must validate against the exact compatibility contract
    # it was built with, even while it is compiling a checkout that advances a
    # breaking alpha ABI. Keep that immutable contract separate from the live
    # release manifest; produced compilers use the live checkout root.
    local frozen_compatibility
    frozen_compatibility="$REPO_ROOT/bootstrap/stage1_frozen.compatibility-manifest.json"
    local frozen_compatibility_hash
    frozen_compatibility_hash="$REPO_ROOT/bootstrap/stage1_frozen.compatibility-manifest.sha256"
    [ -f "$frozen_compatibility" ] && [ ! -L "$frozen_compatibility" ] || {
        echo -e "${RED}ERROR: missing frozen compatibility manifest.${NC}" >&2
        return 1
    }
    [ -f "$frozen_compatibility_hash" ] &&
        [ ! -L "$frozen_compatibility_hash" ] &&
        verify_hash "$frozen_compatibility_hash" || {

        echo -e "${RED}ERROR: frozen compatibility manifest integrity check failed.${NC}" >&2
        return 1
    }
    mkdir -p "$BOOTSTRAP_SOURCE_ROOT/releases"
    for entry in "$REPO_ROOT/releases"/*; do
        [ -e "$entry" ] || continue
        local base
        base=$(basename "$entry")
        if [ "$base" != "compatibility-manifest.json" ]; then
            ln -s "$entry" "$BOOTSTRAP_SOURCE_ROOT/releases/$base"
        fi
    done
    cp -pL "$frozen_compatibility" \
        "$BOOTSTRAP_SOURCE_ROOT/releases/compatibility-manifest.json"
    echo -e "${YELLOW}Bootstrap source view enabled: all Seen source bytes verified unchanged.${NC}"
}

cleanup_bootstrap_source_overlay() {
    if [ -n "$BOOTSTRAP_SOURCE_ROOT" ] && [ "$BOOTSTRAP_SOURCE_ROOT" != "$REPO_ROOT" ]; then
        # Hardened package views are read-only. Restore owner write permission
        # inside the disposable overlay so trap cleanup can remove them.
        chmod -R u+w "$BOOTSTRAP_SOURCE_ROOT/compiler_seen/.seen" 2>/dev/null || true
        rm -rf "$BOOTSTRAP_SOURCE_ROOT"
    fi
}

trap safe_rebuild_cleanup EXIT
trap safe_rebuild_handle_term TERM

extract_expected_module_count() {
    local log_file=$1
    local count=""
    if [ -f "$log_file" ]; then
        count=$(grep -Eo 'Found [0-9]+ modules' "$log_file" 2>/dev/null | head -1 | awk '{print $2}')
    fi
    if [ -z "$count" ]; then
        echo 0
    else
        echo "$count"
    fi
}

tail_log_if_exists() {
    local log_file=$1
    local lines=${2:-30}
    if [ -f "$log_file" ]; then
        tail -"$lines" "$log_file" 2>/dev/null || true
    else
        echo "(missing log: $log_file)"
    fi
}

summarize_stage2_failure_log() {
    local log_file=$1
    if [ ! -f "$log_file" ]; then
        echo "(missing Stage2 log: $log_file)"
        return 0
    fi

    echo -e "${YELLOW}First Stage2 failure signals:${NC}"
    local matches
    matches=$(grep -n -E 'IR VERIFY|llvm-as:|/usr/bin/opt:|clang: error|ld.lld: error|LLVM ERROR|Error: optimization failed|error:' "$log_file" 2>/dev/null | head -40 || true)
    if [ -n "$matches" ]; then
        echo "$matches"
    else
        echo "(no targeted error markers found; tailing recent log output)"
        tail_log_if_exists "$log_file" 40
    fi
}

count_module_objects() {
    local dir=$1
    local count=0
    for f in "$dir"/seen_module_*.o; do
        [ -f "$f" ] || continue
        count=$((count+1))
    done
    echo "$count"
}

count_plain_module_lls() {
    local dir=$1
    local count=0
    for f in "$dir"/seen_module_*.ll; do
        [ -f "$f" ] || continue
        [[ "$f" == *.opt.ll ]] && continue
        [[ "$f" == *.polly.ll ]] && continue
        count=$((count+1))
    done
    echo "$count"
}

find_latest_compile_ll_dir_with_count() {
    local expected_count=$1
    local newer_than=${2:-}
    local latest=""
    local dir
    local count

    for dir in /tmp/seen_compile_*; do
        [ -d "$dir" ] || continue
        if [ -n "$newer_than" ] && [ ! "$dir" -nt "$newer_than" ]; then
            continue
        fi
        count=$(count_plain_module_lls "$dir")
        if [ "$count" -eq "$expected_count" ] 2>/dev/null; then
            if [ -z "$latest" ] || [ "$dir" -nt "$latest" ]; then
                latest="$dir"
            fi
        fi
    done

    echo "$latest"
}

count_module_opt_lls() {
    local dir=$1
    local count=0
    for f in "$dir"/seen_module_*.opt.ll; do
        [ -f "$f" ] || continue
        count=$((count+1))
    done
    echo "$count"
}

list_modules_missing_objects() {
    local dir=$1
    local missing=""
    for llfile in "$dir"/seen_module_*.ll; do
        [ -f "$llfile" ] || continue
        [[ "$llfile" == *.opt.ll ]] && continue
        [[ "$llfile" == *.polly.ll ]] && continue
        local modname=$(basename "$llfile" .ll)
        if [ ! -f "$dir/${modname}.o" ]; then
            missing="$missing ${modname}"
        fi
    done
    echo "$missing"
}

find_problem_empty_modules() {
    local dir=$1
    local empty=""
    for llfile in "$dir"/seen_module_*.ll; do
        [ -f "$llfile" ] || continue
        [[ "$llfile" == *.opt.ll ]] && continue
        [[ "$llfile" == *.polly.ll ]] && continue
        local defines=$(grep -c '^define' "$llfile" 2>/dev/null | tail -1)
        defines=${defines:-0}
        if [ "$defines" -eq 0 ] 2>/dev/null; then
            local strings=$(grep -c '@\.str' "$llfile" 2>/dev/null | tail -1)
            strings=${strings:-0}
            if [ "$strings" -gt 0 ] 2>/dev/null; then
                empty="$empty $(basename "$llfile" .ll)"
            fi
        fi
    done
    echo "$empty"
}

bootstrap_binary_usable() {
    local bin=$1
    local smoke_log="/tmp/seen_bootstrap_smoke_$$_$(basename "$bin").log"
    [ -x "$bin" ] || return 1
    compiler_serializer_applicable "$bin" || return 1
    run_guarded_command "bootstrap smoke $(basename "$bin")" 5 "$MAIN_COMPILER_VMEM_KB" \
        env -u SEEN_FORK_SERIALIZER_ROOT_PID -u SEEN_PACKAGE_CLIENT \
            LD_PRELOAD="$FORK_SERIALIZER_SO" \
            SEEN_FORK_SERIALIZER_TARGET="$bin" \
            "$bin" >"$smoke_log" 2>&1
    local exit_code=$?
    if [ "$exit_code" -ne 0 ] && [ "$exit_code" -ne 1 ]; then
        echo "Bootstrap startup smoke failed for $bin (status $exit_code):" >&2
        sed -n '1,40p' "$smoke_log" >&2 2>/dev/null || true
    fi
    rm -f "$smoke_log"
    [ "$exit_code" -eq 0 ] || [ "$exit_code" -eq 1 ]
}

stage2_failure_looks_oom() {
    local exit_code=$1
    local log_file=$2
    if [ "$exit_code" -eq 137 ]; then
        return 0
    fi
    if [ -f "$log_file" ] && grep -qiE 'killed|out of memory|oom' "$log_file"; then
        return 0
    fi
    return 1
}

preserve_stage2_failure_artifacts() {
    local snapshot_dir=$1
    local preserve_dir="/tmp/seen_stage2_failure_$$"
    rm -rf "$preserve_dir"
    mkdir -p "$preserve_dir"
    cp /tmp/safe_rebuild_stage2.log "$preserve_dir/" 2>/dev/null || true
    cp /tmp/safe_rebuild_stage2.guard.log "$preserve_dir/" 2>/dev/null || true
    if [ -d "$snapshot_dir" ]; then
        cp "$snapshot_dir"/seen_module_* "$preserve_dir/" 2>/dev/null || true
    fi
    cp /tmp/seen_module_* "$preserve_dir/" 2>/dev/null || true
    echo -e "${YELLOW}Preserved Stage2 failure artifacts: $preserve_dir${NC}"
}

is_positive_integer() {
    case "$1" in
        ''|*[!0-9]*) return 1 ;;
        *) [ "$1" -gt 0 ] 2>/dev/null ;;
    esac
}

detect_physical_memory_kb() {
    local mem_kb=""
    if [ -r /proc/meminfo ]; then
        mem_kb=$(awk '/^MemTotal:/ { print $2; exit }' /proc/meminfo 2>/dev/null)
    fi
    if ! is_positive_integer "$mem_kb" && command -v sysctl >/dev/null 2>&1; then
        local mem_bytes
        mem_bytes=$(sysctl -n hw.memsize 2>/dev/null || true)
        if is_positive_integer "$mem_bytes"; then
            mem_kb=$((mem_bytes / 1024))
        fi
    fi
    if is_positive_integer "$mem_kb"; then
        echo "$mem_kb"
    fi
}

detect_available_memory_kb() {
    local mem_kb=""
    if [ -r /proc/meminfo ]; then
        mem_kb=$(awk '/^MemAvailable:/ { print $2; exit }' /proc/meminfo 2>/dev/null)
    fi
    if is_positive_integer "$mem_kb"; then
        echo "$mem_kb"
    fi
}

detect_cgroup_memory_kb() {
    local limit_bytes=""
    if [ -r /sys/fs/cgroup/memory.max ]; then
        limit_bytes=$(cat /sys/fs/cgroup/memory.max 2>/dev/null || true)
    elif [ -r /sys/fs/cgroup/memory/memory.limit_in_bytes ]; then
        limit_bytes=$(cat /sys/fs/cgroup/memory/memory.limit_in_bytes 2>/dev/null || true)
    fi

    if [ "$limit_bytes" = "max" ] || ! is_positive_integer "$limit_bytes"; then
        return 0
    fi

    # Some cgroup v1 hosts report a huge sentinel when no limit is configured.
    if [ "$limit_bytes" -ge 1125899906842624 ] 2>/dev/null; then
        return 0
    fi

    echo $((limit_bytes / 1024))
}

detect_effective_system_memory_kb() {
    local physical_kb
    local cgroup_kb
    physical_kb=$(detect_physical_memory_kb)
    cgroup_kb=$(detect_cgroup_memory_kb)

    if ! is_positive_integer "$physical_kb"; then
        return 1
    fi

    if is_positive_integer "$cgroup_kb" && [ "$cgroup_kb" -lt "$physical_kb" ]; then
        echo "$cgroup_kb"
    else
        echo "$physical_kb"
    fi
}

derive_main_compiler_vmem_kb() {
    local total_kb=$1
    local available_kb=$2
    local cap_kb=$((total_kb * 60 / 100))

    if is_positive_integer "$available_kb"; then
        local reserve_kb=$((total_kb * 10 / 100))
        local available_cap_kb=$((available_kb - reserve_kb))
        if [ "$available_cap_kb" -lt 1 ]; then
            available_cap_kb=$((available_kb / 2))
        fi
        if [ "$available_cap_kb" -gt 0 ] && [ "$available_cap_kb" -lt "$cap_kb" ]; then
            cap_kb=$available_cap_kb
        fi
    fi

    if [ "$cap_kb" -lt 1 ]; then
        cap_kb=1
    fi

    echo "$cap_kb"
}

derive_opt_vmem_kb() {
    local total_kb=$1
    local main_kb=$2
    local cap_kb=$((total_kb * 10 / 100))
    local half_main_kb=$((main_kb / 2))

    if [ "$half_main_kb" -gt 0 ] && [ "$half_main_kb" -lt "$cap_kb" ]; then
        cap_kb=$half_main_kb
    fi

    local max_opt_kb=$((2 * 1024 * 1024))
    if [ "$cap_kb" -gt "$max_opt_kb" ]; then
        cap_kb=$max_opt_kb
    fi

    if [ "$cap_kb" -lt 1 ]; then
        cap_kb=1
    fi

    echo "$cap_kb"
}

derive_memory_guard_reserve_kb() {
    local total_kb=$1
    local available_kb=$2
    local reserve_kb=$((total_kb * 10 / 100))
    local min_reserve_kb=$((4 * 1024 * 1024))

    if [ "$reserve_kb" -lt "$min_reserve_kb" ]; then
        reserve_kb=$min_reserve_kb
    fi

    if is_positive_integer "$available_kb"; then
        local half_available_kb=$((available_kb / 2))
        if [ "$half_available_kb" -gt 0 ] && [ "$reserve_kb" -gt "$half_available_kb" ]; then
            reserve_kb=$half_available_kb
        fi
    fi

    if [ "$reserve_kb" -lt 1 ]; then
        reserve_kb=1
    fi

    echo "$reserve_kb"
}

derive_memory_guard_rss_kb() {
    local total_kb=$1
    local available_kb=$2
    local reserve_kb=${3:-$((total_kb * 10 / 100))}
    local cap_kb=$((total_kb * 60 / 100))

    if is_positive_integer "$available_kb"; then
        local available_cap_kb=$((available_kb - reserve_kb))
        if [ "$available_cap_kb" -lt 1 ]; then
            available_cap_kb=$((available_kb / 2))
        fi
        if [ "$available_cap_kb" -gt 0 ] && [ "$available_cap_kb" -lt "$cap_kb" ]; then
            cap_kb=$available_cap_kb
        fi
    fi

    if [ "$cap_kb" -lt 1 ]; then
        cap_kb=1
    fi

    echo "$cap_kb"
}

guard_low_memory_concurrency() {
    if [ "${SEEN_ALLOW_CONCURRENT_REBUILD:-0}" = "1" ]; then
        return 0
    fi

    local matches
    matches=$(ps -eo pid=,ppid=,comm=,args= | awk -v self="$$" '
        $1 == self { next }
        $2 == self { next }
        $3 == "safe_rebuild.sh" { print; next }
        $0 ~ /(^|[[:space:]])seen[[:space:]]+compile([[:space:]]|$)/ { print; next }
        $0 ~ /compiler_seen\/target\/seen[[:space:]]+compile([[:space:]]|$)/ { print; next }
        $0 ~ /seen_preserved_prod_builder[[:space:]]+compile([[:space:]]|$)/ { print; next }
    ')

    if [ -n "$matches" ]; then
        echo -e "${RED}ERROR: Low-memory rebuild refused because another Seen compile/rebuild appears to be running.${NC}"
        echo "Set SEEN_ALLOW_CONCURRENT_REBUILD=1 only if you have checked memory pressure manually."
        echo "$matches"
        exit 1
    fi
}

configure_adaptive_rebuild_workers() {
    if [ -n "${SEEN_JOBS:-}" ] && ! is_positive_integer "$SEEN_JOBS"; then
        echo -e "${RED}ERROR: SEEN_JOBS must be a positive integer.${NC}" >&2
        exit 1
    fi
    if [ -n "${SEEN_OPT_JOBS:-}" ] && ! is_positive_integer "$SEEN_OPT_JOBS"; then
        echo -e "${RED}ERROR: SEEN_OPT_JOBS must be a positive integer.${NC}" >&2
        exit 1
    fi
    if [ -n "${SEEN_JOBS:-}" ] && [ "$SEEN_JOBS" -ne 1 ]; then
        echo -e "${RED}ERROR: hard-contained rebuilds require SEEN_JOBS=1.${NC}" >&2
        exit 1
    fi
    if [ -n "${SEEN_OPT_JOBS:-}" ] && [ "$SEEN_OPT_JOBS" -ne 1 ]; then
        echo -e "${RED}ERROR: hard-contained rebuilds require SEEN_OPT_JOBS=1.${NC}" >&2
        exit 1
    fi
    export SEEN_JOBS=1
    export SEEN_OPT_JOBS=1

    if declare -F seen_build_trace_event >/dev/null 2>&1; then
        seen_build_trace_event "worker budget" "ok" "SEEN_JOBS=$SEEN_JOBS SEEN_OPT_JOBS=$SEEN_OPT_JOBS"
    fi
}

clean_rebuild_caches() {
    local reason="${1:-requested}"
    local trace_start=""
    if declare -F seen_build_trace_step_start >/dev/null 2>&1; then
        trace_start=$(seen_build_trace_step_start "cache cleanup")
    fi
    rm -rf .seen_cache/ /tmp/seen_ir_cache/ /tmp/seen_thinlto_cache/ /tmp/seen_testmain_obj_cache/
    if declare -F seen_build_trace_step_end >/dev/null 2>&1; then
        seen_build_trace_step_end "cache cleanup" "$trace_start" "ok" "$reason"
    fi
}

echo "=== Safe Rebuild Script ==="
echo ""
echo "Tier: $REBUILD_TIER"
if [ "$CLEAN_CACHE" = "1" ]; then
    echo "Cache cleanup: requested"
fi

# Detect host platform
HOST_OS=$(uname -s)
HOST_ARCH=$(uname -m)
LOW_MEMORY_MODE=0
STAGE2_COMPILE_FLAGS="--fast --no-cache"
PASS2_COMPILE_FLAGS="--fast --no-cache"
RELEASE_CPU_BASELINE="${SEEN_RELEASE_CPU_BASELINE:-}"
RELEASE_TARGET_CPU_FLAG=""
RELEASE_CLANG_MARCH_FLAG="$(release_cpu_baseline_to_march "$RELEASE_CPU_BASELINE")"
MAIN_COMPILER_VMEM_KB=""
OPT_VMEM_KB=""
RECOVERY_TIMEOUT_SECS="${SEEN_RECOVERY_TIMEOUT_SECS:-1800}"
TIER_TIMEOUT_SECS="${SEEN_TIER_TIMEOUT_SECS:-2700}"
IR_RECOVERY_DISABLED="${SEEN_DISABLE_IR_RECOVERY:-0}"
# Internal capability marker. Never accept it from the caller; the exact
# frozen Stage-1 command and captured-frozen-IR recovery set it explicitly.
unset SEEN_FROZEN_IR_COMPAT
SYSTEM_MEMORY_KB=$(detect_effective_system_memory_kb || true)
SYSTEM_AVAILABLE_KB=$(detect_available_memory_kb || true)
MEMORY_GUARD_RSS_KB=""
MEMORY_GUARD_RESERVE_KB=""
MEMORY_GUARD_TASKS_MAX=""
MEMORY_GUARD_CGROUP_STOP_KB=""

if [ "${SEEN_DISABLE_MEMORY_GUARD:-0}" = "1" ]; then
    echo -e "${RED}ERROR: safe rebuilds may not disable hard memory containment.${NC}" >&2
    exit 1
fi
if [ ! -x "$MEMORY_GUARD_SCRIPT" ]; then
    echo -e "${RED}ERROR: required memory guard is missing or not executable: $MEMORY_GUARD_SCRIPT${NC}" >&2
    exit 1
fi
if [ ! -f "$BUILDER_CAPABILITY_SCRIPT" ]; then
    echo -e "${RED}ERROR: required builder-capability helper is missing: $BUILDER_CAPABILITY_SCRIPT${NC}" >&2
    exit 1
fi
if [ ! -f "$BUILDER_SELECTION_SCRIPT" ]; then
    echo -e "${RED}ERROR: required builder-selection helper is missing: $BUILDER_SELECTION_SCRIPT${NC}" >&2
    exit 1
fi
case "${SEEN_LOW_MEMORY:-1}" in
    1) ;;
    *)
        echo -e "${RED}ERROR: safe rebuilds require capped mode; SEEN_LOW_MEMORY must be 1.${NC}" >&2
        exit 1
        ;;
esac
SEEN_LOW_MEMORY=1
export SEEN_LOW_MEMORY

if [ -n "$RELEASE_CPU_BASELINE" ]; then
    RELEASE_TARGET_CPU_FLAG="--target-cpu=$RELEASE_CPU_BASELINE"
    STAGE2_COMPILE_FLAGS="$STAGE2_COMPILE_FLAGS $RELEASE_TARGET_CPU_FLAG"
    PASS2_COMPILE_FLAGS="$PASS2_COMPILE_FLAGS $RELEASE_TARGET_CPU_FLAG"
    export SEEN_RELEASE_CPU_BASELINE="$RELEASE_CPU_BASELINE"
    echo -e "${YELLOW}Release CPU baseline enabled: $RELEASE_CPU_BASELINE.${NC}"
fi

if [ "${SEEN_DISABLE_MEMORY_GUARD:-0}" != "1" ]; then
    if ! is_positive_integer "$SYSTEM_MEMORY_KB"; then
        echo -e "${RED}ERROR: Could not detect system memory for rebuild memory guard.${NC}"
        echo "Set SEEN_GUARD_RSS_KB and SEEN_GUARD_RESERVE_KB explicitly, or run on a host with /proc/meminfo or sysctl hw.memsize."
        exit 1
    fi
    if [ -n "${SEEN_GUARD_RSS_KB:-}" ] && ! is_positive_integer "$SEEN_GUARD_RSS_KB"; then
        echo -e "${RED}ERROR: SEEN_GUARD_RSS_KB must be a positive integer KB value.${NC}"
        exit 1
    fi
    if [ -n "${SEEN_GUARD_RESERVE_KB:-}" ] && ! is_positive_integer "$SEEN_GUARD_RESERVE_KB"; then
        echo -e "${RED}ERROR: SEEN_GUARD_RESERVE_KB must be a positive integer KB value.${NC}"
        exit 1
    fi
    if [ -n "${SEEN_GUARD_TASKS_MAX:-}" ] && ! is_positive_integer "$SEEN_GUARD_TASKS_MAX"; then
        echo -e "${RED}ERROR: SEEN_GUARD_TASKS_MAX must be a positive integer value.${NC}"
        exit 1
    fi
    if [ -n "${SEEN_GUARD_CGROUP_STOP_KB:-}" ] && ! is_positive_integer "$SEEN_GUARD_CGROUP_STOP_KB"; then
        echo -e "${RED}ERROR: SEEN_GUARD_CGROUP_STOP_KB must be a positive integer KB value.${NC}"
        exit 1
    fi
    MEMORY_GUARD_RESERVE_KB="${SEEN_GUARD_RESERVE_KB:-$(derive_memory_guard_reserve_kb "$SYSTEM_MEMORY_KB" "$SYSTEM_AVAILABLE_KB")}"
    DERIVED_MEMORY_CAP_KB=$(derive_memory_guard_rss_kb "$SYSTEM_MEMORY_KB" "$SYSTEM_AVAILABLE_KB" "$MEMORY_GUARD_RESERVE_KB")
    MEMORY_GUARD_RSS_KB="${SEEN_GUARD_RSS_KB:-$DERIVED_MEMORY_CAP_KB}"
    MEMORY_GUARD_TASKS_MAX="${SEEN_GUARD_TASKS_MAX:-24}"
    MEMORY_GUARD_CGROUP_STOP_KB="${SEEN_GUARD_CGROUP_STOP_KB:-$MEMORY_GUARD_RSS_KB}"
    if [ "$MEMORY_GUARD_CGROUP_STOP_KB" -lt 1 ]; then
        MEMORY_GUARD_CGROUP_STOP_KB=1
    fi
    export SEEN_MEMORY_GUARD_RSS_KB="$MEMORY_GUARD_RSS_KB"
    export SEEN_MEMORY_GUARD_RESERVE_KB="$MEMORY_GUARD_RESERVE_KB"
    export SEEN_MEMORY_GUARD_TASKS_MAX="$MEMORY_GUARD_TASKS_MAX"
    export SEEN_MEMORY_GUARD_CGROUP_STOP_KB="$MEMORY_GUARD_CGROUP_STOP_KB"
    if [ "$MEMORY_GUARD_RSS_KB" -gt "$DERIVED_MEMORY_CAP_KB" ]; then
        echo -e "${RED}ERROR: SEEN_GUARD_RSS_KB may not exceed the current-memory-derived cap ($DERIVED_MEMORY_CAP_KB KiB).${NC}" >&2
        exit 1
    fi
    if [ "$MEMORY_GUARD_TASKS_MAX" -gt 24 ]; then
        echo -e "${RED}ERROR: SEEN_GUARD_TASKS_MAX may not exceed the hard rebuild ceiling of 24.${NC}" >&2
        exit 1
    fi
    if [ "$HOST_OS" != "Linux" ]; then
        echo -e "${RED}ERROR: no equally hard rebuild memory scope is implemented for $HOST_OS.${NC}" >&2
        exit 1
    fi
    if [ "${SEEN_MEMORY_GUARD_KERNEL_SCOPE:-1}" = "0" ]; then
        echo -e "${RED}ERROR: Linux safe rebuilds require a kernel cgroup; polling-only containment is forbidden.${NC}" >&2
        exit 1
    fi
    if ! user_memory_scope_available; then
        echo -e "${RED}ERROR: cannot create the required user systemd memory scope.${NC}" >&2
        echo "No compiler or helper build was started. Run from a session with a working user systemd manager." >&2
        exit 1
    fi
    export SEEN_MEMORY_GUARD_KERNEL_SCOPE=1
    export SEEN_MEMORY_GUARD_REQUIRE_KERNEL_SCOPE=1
fi

if ! is_positive_integer "$TIER_TIMEOUT_SECS"; then
    echo -e "${RED}ERROR: SEEN_TIER_TIMEOUT_SECS must be a positive integer number of seconds.${NC}" >&2
    exit 1
fi

if [ "${SEEN_LOW_MEMORY:-0}" = "1" ]; then
    LOW_MEMORY_MODE=1
    if ! is_positive_integer "$SYSTEM_MEMORY_KB"; then
        echo -e "${RED}ERROR: Could not detect system memory for low-memory rebuild caps.${NC}"
        echo "Set SEEN_MAIN_VMEM_KB and SEEN_OPT_VMEM_KB explicitly, or run on a host with /proc/meminfo or sysctl hw.memsize."
        exit 1
    fi
    if [ -n "${SEEN_MAIN_VMEM_KB:-}" ] && ! is_positive_integer "$SEEN_MAIN_VMEM_KB"; then
        echo -e "${RED}ERROR: SEEN_MAIN_VMEM_KB must be a positive integer KB value.${NC}"
        exit 1
    fi
    if [ -n "${SEEN_OPT_VMEM_KB:-}" ] && ! is_positive_integer "$SEEN_OPT_VMEM_KB"; then
        echo -e "${RED}ERROR: SEEN_OPT_VMEM_KB must be a positive integer KB value.${NC}"
        exit 1
    fi
    MAIN_COMPILER_VMEM_KB="${SEEN_MAIN_VMEM_KB:-$(derive_main_compiler_vmem_kb "$SYSTEM_MEMORY_KB" "$SYSTEM_AVAILABLE_KB")}"
    OPT_VMEM_KB="${SEEN_OPT_VMEM_KB:-$(derive_opt_vmem_kb "$SYSTEM_MEMORY_KB" "$MAIN_COMPILER_VMEM_KB")}"
    if ! is_positive_integer "$MAIN_COMPILER_VMEM_KB" || ! is_positive_integer "$OPT_VMEM_KB"; then
        echo -e "${RED}ERROR: Low-memory rebuild caps must be positive integer KB values.${NC}"
        exit 1
    fi
    if [ "$MAIN_COMPILER_VMEM_KB" -gt "$DERIVED_MEMORY_CAP_KB" ]; then
        echo -e "${RED}ERROR: SEEN_MAIN_VMEM_KB may not exceed the current-memory-derived cap ($DERIVED_MEMORY_CAP_KB KiB).${NC}" >&2
        exit 1
    fi
    if [ "$OPT_VMEM_KB" -gt 2097152 ]; then
        echo -e "${RED}ERROR: SEEN_OPT_VMEM_KB may not exceed the 2 GiB hard ceiling (2097152 KiB).${NC}" >&2
        exit 1
    fi
    if [ "$OPT_VMEM_KB" -gt "$MAIN_COMPILER_VMEM_KB" ]; then
        echo -e "${RED}ERROR: SEEN_OPT_VMEM_KB may not exceed SEEN_MAIN_VMEM_KB.${NC}" >&2
        exit 1
    fi
    if [ -z "${SEEN_GUARD_RSS_KB:-}" ]; then
        MEMORY_GUARD_RSS_KB="$MAIN_COMPILER_VMEM_KB"
        export SEEN_MEMORY_GUARD_RSS_KB="$MEMORY_GUARD_RSS_KB"
    fi
    if [ -z "${SEEN_GUARD_TASKS_MAX:-}" ]; then
        MEMORY_GUARD_TASKS_MAX=24
        export SEEN_MEMORY_GUARD_TASKS_MAX="$MEMORY_GUARD_TASKS_MAX"
    fi
    if [ -z "${SEEN_GUARD_CGROUP_STOP_KB:-}" ]; then
        MEMORY_GUARD_CGROUP_STOP_KB=$MEMORY_GUARD_RSS_KB
        if [ "$MEMORY_GUARD_CGROUP_STOP_KB" -lt 1 ]; then
            MEMORY_GUARD_CGROUP_STOP_KB=1
        fi
        export SEEN_MEMORY_GUARD_CGROUP_STOP_KB="$MEMORY_GUARD_CGROUP_STOP_KB"
    fi
    if [ "$MEMORY_GUARD_RSS_KB" -gt "$MAIN_COMPILER_VMEM_KB" ]; then
        echo -e "${RED}ERROR: SEEN_GUARD_RSS_KB may not exceed SEEN_MAIN_VMEM_KB.${NC}" >&2
        exit 1
    fi
    max_early_stop_kb=$MEMORY_GUARD_RSS_KB
    if [ "$MEMORY_GUARD_CGROUP_STOP_KB" -gt "$max_early_stop_kb" ]; then
        MEMORY_GUARD_CGROUP_STOP_KB="$max_early_stop_kb"
        export SEEN_MEMORY_GUARD_CGROUP_STOP_KB="$MEMORY_GUARD_CGROUP_STOP_KB"
    fi
    RECOVERY_TIMEOUT_SECS="${SEEN_RECOVERY_TIMEOUT_SECS:-7200}"
    MAIN_COMPILER_MEMORY_LIMIT_BYTES="${SEEN_MEMORY_LIMIT_BYTES:-$((MAIN_COMPILER_VMEM_KB * 1024))}"
    if ! is_positive_integer "$MAIN_COMPILER_MEMORY_LIMIT_BYTES" ||
        [ "$MAIN_COMPILER_MEMORY_LIMIT_BYTES" -gt "$((MAIN_COMPILER_VMEM_KB * 1024))" ]; then

        echo -e "${RED}ERROR: SEEN_MEMORY_LIMIT_BYTES must be positive and no larger than SEEN_MAIN_VMEM_KB in bytes.${NC}" >&2
        exit 1
    fi
    export SEEN_LOW_MEMORY=1
    export SEEN_MAIN_VMEM_KB="$MAIN_COMPILER_VMEM_KB"
    export SEEN_OPT_VMEM_KB="$OPT_VMEM_KB"
    export SEEN_MEMORY_LIMIT_BYTES="$MAIN_COMPILER_MEMORY_LIMIT_BYTES"
    export SEEN_RECOVERY_TIMEOUT_SECS="$RECOVERY_TIMEOUT_SECS"
    guard_low_memory_concurrency
    if [ "$REBUILD_TIER" = "full" ]; then
        echo -e "${YELLOW}Low-memory mode enabled: serial full-bootstrap stages.${NC}"
    else
        echo -e "${YELLOW}Low-memory mode enabled: adaptive bounded worker tiers.${NC}"
    fi
    echo -e "${YELLOW}Detected system memory: $(format_bytes $((SYSTEM_MEMORY_KB * 1024))). Main compiler cap: $(format_bytes $((MAIN_COMPILER_VMEM_KB * 1024))). tracked allocation budget: $(format_bytes "$MAIN_COMPILER_MEMORY_LIMIT_BYTES"). opt cap: $(format_bytes $((OPT_VMEM_KB * 1024))).${NC}"
fi

configure_adaptive_rebuild_workers

if [ "${SEEN_REBUILD_AGGREGATE_SCOPE_ACTIVE:-0}" != "1" ]; then
    if [ "${SEEN_CI_CONTAINMENT_IN_SCOPE:-0}" = "1" ] ||
        [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" = "1" ] ||
        [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" = "1" ]; then

        # Required CI already owns an aggregate kernel scope. Accept it only
        # when every marker is present and the hard-scope helper independently
        # reads back the requested memory, swap, task, and worker limits.
        if [ "${SEEN_CI_CONTAINMENT_IN_SCOPE:-0}" != "1" ] ||
            [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != "1" ] ||
            [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" != "1" ] ||
            ! "$HARD_MEMORY_SCOPE_WRAPPER" \
                --label "safe rebuild containing CI read-back" \
                --verify-only -- >/dev/null; then

            echo -e "${RED}ERROR: containing CI scope markers or kernel read-back are invalid.${NC}" >&2
            echo "No compiler or helper build was started." >&2
            exit 126
        fi
        SEEN_REBUILD_AGGREGATE_SCOPE_ACTIVE=1
        export SEEN_REBUILD_AGGREGATE_SCOPE_ACTIVE
        echo -e "${YELLOW}Using the read-back-verified containing CI cgroup as the rebuild aggregate scope.${NC}"
    else
        prepare_bounded_wine_prefix_template || exit $?
        enter_rebuild_kernel_scope
    fi
fi
if [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" != "1" ]; then
    echo -e "${RED}ERROR: aggregate rebuild cgroup marker was forged or lost.${NC}" >&2
    echo "No compiler or helper build was started." >&2
    exit 1
fi
if ! verify_rebuild_kernel_scope; then
    echo -e "${RED}ERROR: hard rebuild containment preflight failed.${NC}" >&2
    echo "No compiler or helper build was started." >&2
    exit 1
fi
verified_cgroup_path=$(awk -F= '$1 == "verified_cgroup_path" { sub(/^[^=]*=/, ""); print; exit }' "$REBUILD_SCOPE_EVIDENCE_FILE")
verified_memory_max=$(awk -F= '$1 == "verified_memory_max_bytes" { print $2; exit }' "$REBUILD_SCOPE_EVIDENCE_FILE")
verified_memory_swap_max=$(awk -F= '$1 == "verified_memory_swap_max_bytes" { print $2; exit }' "$REBUILD_SCOPE_EVIDENCE_FILE")
verified_pids_max=$(awk -F= '$1 == "verified_pids_max" { print $2; exit }' "$REBUILD_SCOPE_EVIDENCE_FILE")
if [ -z "$verified_cgroup_path" ] || ! is_positive_integer "$verified_memory_max" ||
    [ "$verified_memory_swap_max" != "0" ] || ! is_positive_integer "$verified_pids_max"; then

    echo -e "${RED}ERROR: verified cgroup evidence record is missing or incomplete.${NC}" >&2
    exit 1
fi
echo -e "${YELLOW}Verified kernel containment: cgroup $verified_cgroup_path; memory.max $verified_memory_max bytes; memory.swap.max $verified_memory_swap_max; pids.max $verified_pids_max.${NC}"
SEEN_REBUILD_AGGREGATE_SCOPE_VERIFIED=1
export SEEN_REBUILD_AGGREGATE_SCOPE_VERIFIED

# Build the root-targeted serializer only after actual cgroup read-back, then
# require it for every compiler candidate in every tier. It serializes direct
# compiler children; the hard cgroup, task cap, serial worker settings, and
# bounded tool wrappers remain authoritative for descendant tools. Advertised
# CLI flags are defense in depth, never proof that a binary honors them.
build_fork_serializer || exit 1
prepare_bounded_toolchain || exit 1
safe_rebuild_validate_install_destinations || exit 1

if memory_guard_enabled; then
    echo -e "${YELLOW}Memory guard enabled: tree RSS cap $(format_bytes $((MEMORY_GUARD_RSS_KB * 1024))); cgroup stop $(format_bytes $((MEMORY_GUARD_CGROUP_STOP_KB * 1024))); reserve $(format_bytes $((MEMORY_GUARD_RESERVE_KB * 1024))); tasks max ${MEMORY_GUARD_TASKS_MAX:-unlimited}.${NC}"
fi
if [ "$IR_RECOVERY_DISABLED" = "1" ]; then
    echo -e "${YELLOW}Strict rebuild validation enabled: direct IR recovery is disabled.${NC}"
fi

if [ "$HOST_OS" = "Darwin" ]; then
    # macOS
    if [ "$HOST_ARCH" = "arm64" ]; then
        FROZEN="bootstrap/stage1_frozen_macos_arm64"
        HASH_FILE="bootstrap/stage1_frozen_macos_arm64.sha256"
        echo "Detected macOS ARM64 (Apple Silicon), using stage1_frozen_macos_arm64"
    else
        FROZEN="bootstrap/stage1_frozen_macos_x86_64"
        HASH_FILE="bootstrap/stage1_frozen_macos_x86_64.sha256"
        echo "Detected macOS x86_64, using stage1_frozen_macos_x86_64"
    fi
else
    # Linux: Auto-detect CPU ISA level and select appropriate bootstrap
    detect_isa_level() {
        if grep -q 'avx512f' /proc/cpuinfo 2>/dev/null; then
            echo "v4"
        else
            echo "v3"
        fi
    }

    ISA_LEVEL=$(detect_isa_level)
    if [ "$ISA_LEVEL" = "v4" ]; then
        FROZEN="bootstrap/stage1_frozen"
        HASH_FILE="bootstrap/stage1_frozen.sha256"
        echo "Detected x86-64-v4 (AVX-512) CPU, using stage1_frozen"
    else
        FROZEN="bootstrap/stage1_frozen_v3"
        HASH_FILE="bootstrap/stage1_frozen_v3.sha256"
        echo "Detected x86-64-v3 CPU, using stage1_frozen_v3"
    fi
fi

# Check frozen compiler exists
if [ "$REBUILD_TIER" = "full" ]; then
    if [ ! -f "$FROZEN" ]; then
        echo -e "${RED}ERROR: Frozen compiler not found at $FROZEN${NC}"
        echo "Run this script from the repository root."
        if [ "$HOST_OS" = "Darwin" ]; then
            echo "On macOS, run scripts/bootstrap_macos.sh first to create the macOS bootstrap."
        fi
        exit 1
    fi

    if ! bootstrap_binary_usable "$FROZEN"; then
        if [ "$HOST_OS" != "Darwin" ] && [ "$FROZEN" = "bootstrap/stage1_frozen" ] && [ -x "bootstrap/stage1_frozen_v3" ] && bootstrap_binary_usable "bootstrap/stage1_frozen_v3"; then
            echo -e "${YELLOW}bootstrap/stage1_frozen failed a startup smoke test; falling back to bootstrap/stage1_frozen_v3.${NC}"
            FROZEN="bootstrap/stage1_frozen_v3"
            HASH_FILE="bootstrap/stage1_frozen_v3.sha256"
        else
            echo -e "${RED}ERROR: Frozen compiler at $FROZEN failed a startup smoke test.${NC}"
            exit 1
        fi
    fi
elif declare -F seen_build_trace_event >/dev/null 2>&1; then
    seen_build_trace_event "bootstrap preflight" "deferred" "$REBUILD_TIER tier"
fi

# Verify frozen compiler hash (cross-platform)
verify_hash() {
    if command -v sha256sum &>/dev/null; then
        sha256sum -c "$1" > /dev/null 2>&1
    elif command -v shasum &>/dev/null; then
        shasum -a 256 -c "$1" > /dev/null 2>&1
    else
        echo -e "${YELLOW}WARNING: No sha256sum or shasum found, skipping hash verification${NC}"
        return 0
    fi
}

ensure_bootstrap_preflight() {
    if [ "$BOOTSTRAP_PREFLIGHT_DONE" = "1" ]; then
        return 0
    fi

    local trace_start=""
    if declare -F seen_build_trace_step_start >/dev/null 2>&1; then
        trace_start=$(seen_build_trace_step_start "bootstrap preflight")
    fi

    if [ ! -f "$FROZEN" ]; then
        echo -e "${RED}ERROR: Frozen compiler not found at $FROZEN${NC}"
        echo "Run this script from the repository root."
        if [ "$HOST_OS" = "Darwin" ]; then
            echo "On macOS, run scripts/bootstrap_macos.sh first to create the macOS bootstrap."
        fi
        if declare -F seen_build_trace_step_end >/dev/null 2>&1; then
            seen_build_trace_step_end "bootstrap preflight" "$trace_start" "failed" "missing=$FROZEN"
        fi
        return 1
    fi

    if ! bootstrap_binary_usable "$FROZEN"; then
        if [ "$HOST_OS" != "Darwin" ] && [ "$FROZEN" = "bootstrap/stage1_frozen" ] && [ -x "bootstrap/stage1_frozen_v3" ] && bootstrap_binary_usable "bootstrap/stage1_frozen_v3"; then
            echo -e "${YELLOW}bootstrap/stage1_frozen failed a startup smoke test; falling back to bootstrap/stage1_frozen_v3.${NC}"
            FROZEN="bootstrap/stage1_frozen_v3"
            HASH_FILE="bootstrap/stage1_frozen_v3.sha256"
        else
            echo -e "${RED}ERROR: Frozen compiler at $FROZEN failed a startup smoke test.${NC}"
            if declare -F seen_build_trace_step_end >/dev/null 2>&1; then
                seen_build_trace_step_end "bootstrap preflight" "$trace_start" "failed" "startup=$FROZEN"
            fi
            return 1
        fi
    fi

    echo "Verifying frozen compiler integrity..."
    if verify_hash "$HASH_FILE"; then
        echo -e "${GREEN}Frozen compiler verified.${NC}"
    else
        echo -e "${RED}ERROR: Frozen compiler hash verification failed!${NC}"
        echo "The bootstrap compiler may be corrupted."
        if declare -F seen_build_trace_step_end >/dev/null 2>&1; then
            seen_build_trace_step_end "bootstrap preflight" "$trace_start" "failed" "hash=$HASH_FILE"
        fi
        return 1
    fi

    prepare_bootstrap_source_overlay
    if [ "$BOOTSTRAP_SOURCE_ROOT" != "$REPO_ROOT" ] && [ -f "$BOOTSTRAP_SOURCE_ROOT/$FROZEN" ]; then
        FROZEN_ABS="$BOOTSTRAP_SOURCE_ROOT/$FROZEN"
    else
        FROZEN_ABS="$REPO_ROOT/$FROZEN"
    fi
    BOOTSTRAP_PREFLIGHT_DONE=1
    if declare -F seen_build_trace_step_end >/dev/null 2>&1; then
        seen_build_trace_step_end "bootstrap preflight" "$trace_start" "ok" "frozen=$FROZEN source_root=$BOOTSTRAP_SOURCE_ROOT"
    fi
}

cleanup_smoke_build_state() {
    rm -f /tmp/seen_module_*.ll /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll
    rm -f /tmp/seen_module_*.polly.ll /tmp/seen_module_*.opt.status /tmp/seen_module_*.opt.log
    rm -f /tmp/seen_module_*.relink.o /tmp/safe_rebuild_smoke_bin
    rm -f /tmp/safe_rebuild_*_hello_english.seen
}

preserve_smoke_failure_artifacts() {
    local stage_slug=$1
    local artifact_dir="/tmp/seen_smoke_failure_${stage_slug}_$(date +%s)"
    mkdir -p "$artifact_dir" 2>/dev/null || return 0
    for f in /tmp/seen_module_*.ll /tmp/seen_module_*.opt.ll \
        /tmp/seen_module_*.opt.log /tmp/seen_module_*.opt.status \
        /tmp/safe_rebuild_"$stage_slug"_hello_*.log \
        /tmp/safe_rebuild_"$stage_slug"_hello_english.seen; do
        if [ -e "$f" ]; then
            cp "$f" "$artifact_dir/" 2>/dev/null || true
        fi
    done
    echo -e "${YELLOW}Preserved smoke failure artifacts: $artifact_dir${NC}"
}

preserve_existing_production_compiler() {
    rm -f "$PRESERVED_PROD_BUILDER"
    if [ -x compiler_seen/target/seen ]; then
        cp compiler_seen/target/seen "$PRESERVED_PROD_BUILDER"
        chmod +x "$PRESERVED_PROD_BUILDER"
        return 0
    fi
    if [ -x target/release/seen ]; then
        cp target/release/seen "$PRESERVED_PROD_BUILDER"
        chmod +x "$PRESERVED_PROD_BUILDER"
        return 0
    fi
    return 1
}

hash_paths_for_cache() {
    if declare -F seen_build_hash_paths >/dev/null 2>&1; then
        seen_build_hash_paths "$@"
        return
    fi
    local path
    for path in "$@"; do
        [ -e "$path" ] || continue
        if [ -f "$path" ]; then
            sha256sum "$path"
        else
            find "$path" -type f -print0 | sort -z | xargs -0 sha256sum 2>/dev/null
        fi
    done | sha256sum | awk '{print $1}'
}

smoke_compile_cache_key() {
    local compiler_path=$1
    local smoke_fixture=$2
    local smoke_flags=$3
    local compiler_hash fixture_hash runtime_hash std_hash

    compiler_hash=$(hash_paths_for_cache "$compiler_path")
    fixture_hash=$(hash_paths_for_cache "$smoke_fixture")
    runtime_hash=$(hash_paths_for_cache "$REPO_ROOT/seen_runtime")
    std_hash=$(hash_paths_for_cache "$REPO_ROOT/seen_std/src")
    {
        printf 'smoke-cache-v1\n'
        printf 'compiler=%s\n' "$compiler_hash"
        printf 'fixture=%s\n' "$fixture_hash"
        printf 'runtime=%s\n' "$runtime_hash"
        printf 'stdlib=%s\n' "$std_hash"
        printf 'flags=%s\n' "$smoke_flags"
        printf 'host_os=%s\n' "${HOST_OS:-unknown}"
        printf 'host_arch=%s\n' "${HOST_ARCH:-unknown}"
    } | sha256sum | awk '{print $1}'
}

smoke_cache_default_enabled() {
    case "${SEEN_SMOKE_CACHE:-}" in
        0|false|False|FALSE|no|No|NO)
            return 1
            ;;
        1|true|True|TRUE|yes|Yes|YES)
            return 0
            ;;
    esac
    [ "$REBUILD_TIER" != "full" ]
}

smoke_test_compiler() {
    local compiler_path=$1
    local stage_label=$2
    local stage_slug=$3
    local smoke_fixture="$REPO_ROOT/examples/hello_world/hello_english.seen"
    local smoke_source="/tmp/safe_rebuild_${stage_slug}_hello_english.seen"
    local smoke_bin="/tmp/safe_rebuild_smoke_bin"
    local check_log="/tmp/safe_rebuild_${stage_slug}_hello_check.log"
    local compile_log="/tmp/safe_rebuild_${stage_slug}_hello_compile.log"
    local run_log="/tmp/safe_rebuild_${stage_slug}_hello_run.log"
    local compiler_env=(-u SEEN_PACKAGE_CLIENT
        "SEEN_COMPILER_SOURCE_ROOT=$REPO_ROOT")
    local check_cmd=("$compiler_path" check "$smoke_source")
    local compile_cmd=("$compiler_path" compile "$smoke_source" "$smoke_bin" --fast --no-cache)
    local smoke_flags="--fast --no-cache"
    local smoke_cache_key smoke_cache_dir smoke_cache_bin

    if [ -z "$FORK_SERIALIZER_SO" ] || [ ! -f "$FORK_SERIALIZER_SO" ] ||
        [ -L "$FORK_SERIALIZER_SO" ]; then

        echo -e "${YELLOW}${stage_label} refused: verified fork serializer is unavailable.${NC}"
        return 1
    fi
    if ! compiler_serializer_applicable "$compiler_path"; then
        echo -e "${YELLOW}${stage_label} refused: compiler is not serializer-applicable.${NC}"
        return 1
    fi
    compiler_env+=("LD_PRELOAD=$FORK_SERIALIZER_SO")
    compiler_env+=("SEEN_FORK_SERIALIZER_TARGET=$compiler_path")
    compiler_env+=("SEEN_FORK_SERIALIZER_ROOT_PID=")
    # The fresh source helper is valid only for a compiler that reports the
    # exact checkout version. Legacy/frozen builders inherit no package helper.
    if compiler_reports_checkout_version "$compiler_path"; then
        compiler_env+=("SEEN_PACKAGE_CLIENT=$SOURCE_PACKAGE_CLIENT")
    fi
    if [ "$LOW_MEMORY_MODE" = "1" ]; then
        compiler_env+=("SEEN_LOW_MEMORY=${SEEN_LOW_MEMORY:-1}")
        if [ -n "$MAIN_COMPILER_VMEM_KB" ]; then
            compiler_env+=("SEEN_MAIN_VMEM_KB=$MAIN_COMPILER_VMEM_KB")
        fi
        if [ -n "$OPT_VMEM_KB" ]; then
            compiler_env+=("SEEN_OPT_VMEM_KB=$OPT_VMEM_KB")
        fi
        if [ -n "${SEEN_MEMORY_LIMIT_BYTES:-}" ]; then
            compiler_env+=("SEEN_MEMORY_LIMIT_BYTES=$SEEN_MEMORY_LIMIT_BYTES")
        fi
        if tier_builder_supports_jobs "$compiler_path"; then
            compile_cmd+=(--jobs "$SEEN_JOBS" --opt-jobs "$SEEN_OPT_JOBS")
            smoke_flags="$smoke_flags --jobs $SEEN_JOBS --opt-jobs $SEEN_OPT_JOBS"
        elif tier_builder_supports_no_fork "$compiler_path"; then
            compile_cmd+=(--no-fork)
            smoke_flags="$smoke_flags --no-fork"
        else
            smoke_flags="$smoke_flags serializer-required"
            echo -e "${YELLOW}${stage_label} advertises no worker controls; direct-child serialization and the hard cgroup remain mandatory.${NC}"
        fi
    fi
    if [ -n "${RELEASE_TARGET_CPU_FLAG:-}" ]; then
        compile_cmd+=("$RELEASE_TARGET_CPU_FLAG")
        smoke_flags="$smoke_flags $RELEASE_TARGET_CPU_FLAG"
    fi

    cleanup_smoke_build_state
    if ! cp "$smoke_fixture" "$smoke_source"; then
        echo -e "${YELLOW}${stage_label} could not prepare hello-world smoke source.${NC}"
        return 1
    fi

    if ! (
        cd "$REPO_ROOT" &&
        run_guarded_command_to_log_with_failure_watch "$stage_label check smoke" 120 "$MAIN_COMPILER_VMEM_KB" "$check_log" \
            env "${compiler_env[@]}" "${check_cmd[@]}"
    ); then
        echo -e "${YELLOW}${stage_label} failed hello-world check smoke test.${NC}"
        tail -20 "$check_log" 2>/dev/null || true
        preserve_smoke_failure_artifacts "$stage_slug"
        cleanup_smoke_build_state
        return 1
    fi

    cleanup_smoke_build_state
    if ! cp "$smoke_fixture" "$smoke_source"; then
        echo -e "${YELLOW}${stage_label} could not prepare hello-world smoke source.${NC}"
        return 1
    fi

    if smoke_cache_default_enabled; then
        smoke_cache_key=$(smoke_compile_cache_key "$compiler_path" "$smoke_fixture" "$smoke_flags")
        smoke_cache_dir="$REPO_ROOT/target/seen-build/smoke-cache/$smoke_cache_key"
        smoke_cache_bin="$smoke_cache_dir/safe_rebuild_smoke_bin"
        if [ -x "$smoke_cache_bin" ]; then
            cp "$smoke_cache_bin" "$smoke_bin"
            chmod +x "$smoke_bin" 2>/dev/null || true
            printf 'smoke compile cache hit: %s\n' "$smoke_cache_key" > "$compile_log"
            if declare -F seen_build_trace_event >/dev/null 2>&1; then
                seen_build_trace_event "$stage_label compile smoke cache" "hit" "key=$smoke_cache_key"
            fi
        else
            if declare -F seen_build_trace_event >/dev/null 2>&1; then
                seen_build_trace_event "$stage_label compile smoke cache" "miss" "key=$smoke_cache_key"
            fi
            if ! (
                cd "$REPO_ROOT" &&
                run_guarded_command_to_log_with_failure_watch "$stage_label compile smoke" 120 "$MAIN_COMPILER_VMEM_KB" "$compile_log" \
                    env "${compiler_env[@]}" "${compile_cmd[@]}"
            ); then
                echo -e "${YELLOW}${stage_label} failed hello-world compile smoke test.${NC}"
                tail -20 "$compile_log" 2>/dev/null || true
                preserve_smoke_failure_artifacts "$stage_slug"
                cleanup_smoke_build_state
                return 1
            fi
            if [ -x "$smoke_bin" ]; then
                mkdir -p "$smoke_cache_dir"
                cp "$smoke_bin" "$smoke_cache_bin" 2>/dev/null || true
                chmod +x "$smoke_cache_bin" 2>/dev/null || true
            fi
        fi
    else
        if declare -F seen_build_trace_event >/dev/null 2>&1; then
            seen_build_trace_event "$stage_label compile smoke cache" "disabled" "tier=$REBUILD_TIER"
        fi
        if ! (
            cd "$REPO_ROOT" &&
            run_guarded_command_to_log_with_failure_watch "$stage_label compile smoke" 120 "$MAIN_COMPILER_VMEM_KB" "$compile_log" \
                env "${compiler_env[@]}" "${compile_cmd[@]}"
        ); then
            echo -e "${YELLOW}${stage_label} failed hello-world compile smoke test.${NC}"
            tail -20 "$compile_log" 2>/dev/null || true
            preserve_smoke_failure_artifacts "$stage_slug"
            cleanup_smoke_build_state
            return 1
        fi
    fi

    if [ -f "$smoke_bin" ] && [ ! -x "$smoke_bin" ]; then
        chmod +x "$smoke_bin" 2>/dev/null || true
    fi

    if [ ! -x "$smoke_bin" ]; then
        echo -e "${YELLOW}${stage_label} compile smoke test did not produce executable $smoke_bin.${NC}"
        preserve_smoke_failure_artifacts "$stage_slug"
        cleanup_smoke_build_state
        return 1
    fi

    if ! "$smoke_bin" > "$run_log" 2>&1; then
        echo -e "${YELLOW}${stage_label} failed hello-world run smoke test.${NC}"
        tail -20 "$run_log" 2>/dev/null || true
        preserve_smoke_failure_artifacts "$stage_slug"
        cleanup_smoke_build_state
        return 1
    fi

    echo -e "${GREEN}${stage_label} passed hello-world smoke test.${NC}"
    cleanup_smoke_build_state
    return 0
}

tier_source_root_for_builder() {
    local builder_path=$1
    local builder_name
    builder_name=$(basename "$builder_path")
    if [ "${SEEN_TIER_USE_BOOTSTRAP_OVERLAY:-${SEEN_EXISTING_BUILDER_USE_BOOTSTRAP_OVERLAY:-0}}" = "1" ]; then
        printf '%s\n' "$BOOTSTRAP_SOURCE_ROOT"
        return 0
    fi
    case "$builder_name" in
        seen_frozen*|stage1_frozen*)
            printf '%s\n' "$BOOTSTRAP_SOURCE_ROOT"
            ;;
        *)
            printf '%s\n' "$REPO_ROOT"
            ;;
    esac
}

tier_builder_requires_bootstrap_preflight() {
    if [ "${SEEN_TIER_USE_BOOTSTRAP_OVERLAY:-${SEEN_EXISTING_BUILDER_USE_BOOTSTRAP_OVERLAY:-0}}" = "1" ]; then
        return 0
    fi
    case "$(basename "$1")" in
        seen_frozen*|stage1_frozen*)
            return 0
            ;;
    esac
    return 1
}

tier_builder_path_for_source_root() {
    local builder_path=$1
    local source_root=$2
    if [ "$source_root" = "$BOOTSTRAP_SOURCE_ROOT" ] &&
        [ "${SEEN_TIER_USE_BOOTSTRAP_OVERLAY:-${SEEN_EXISTING_BUILDER_USE_BOOTSTRAP_OVERLAY:-0}}" = "1" ]; then
        local overlay_builder="$BOOTSTRAP_SOURCE_ROOT/compiler_seen/target/seen_tier_builder"
        cp -pL "$builder_path" "$overlay_builder" || return 1
        chmod +x "$overlay_builder" 2>/dev/null || true
        printf '%s\n' "$overlay_builder"
        return 0
    fi
    case "$builder_path" in
        "$REPO_ROOT"/*)
            if [ "$source_root" = "$BOOTSTRAP_SOURCE_ROOT" ]; then
                printf '%s\n' "$BOOTSTRAP_SOURCE_ROOT/${builder_path#$REPO_ROOT/}"
                return 0
            fi
            ;;
    esac
    printf '%s\n' "$builder_path"
}

compiler_reports_checkout_version() {
    local compiler_path=$1
    local version_output version_status first_line

    [ -n "$SOURCE_PACKAGE_CLIENT_VERSION" ] || return 1
    [ -f "$compiler_path" ] && [ -x "$compiler_path" ] &&
        [ ! -L "$compiler_path" ] || return 1
    if version_output=$(env -u LD_PRELOAD -u SEEN_FORK_SERIALIZER_TARGET \
        -u SEEN_FORK_SERIALIZER_ROOT_PID -u SEEN_PACKAGE_CLIENT \
        timeout 10 "$compiler_path" --version 2>&1); then

        version_status=0
    else
        version_status=$?
    fi
    [ "$version_status" -eq 0 ] || return 1
    first_line=${version_output%%$'\n'*}
    [ "$first_line" = "Seen $SOURCE_PACKAGE_CLIENT_VERSION" ]
}

select_tier_builder() {
    local selection_args=(
        --repo-root "$REPO_ROOT"
        --checkout-version "$SOURCE_PACKAGE_CLIENT_VERSION"
        --source-sidecar "$SOURCE_PACKAGE_CLIENT"
    )
    if [ -n "${SEEN_STAGE_BUILDER:-}" ]; then
        selection_args+=(--explicit-builder "$SEEN_STAGE_BUILDER")
    fi
    bash "$BUILDER_SELECTION_SCRIPT" "${selection_args[@]}"
}

tier_builder_supports_jobs() {
    local builder_path=$1
    local capability
    local capability_status=0
    capability=$(env -u LD_PRELOAD -u SEEN_FORK_SERIALIZER_TARGET \
        -u SEEN_FORK_SERIALIZER_ROOT_PID \
        bash "$BUILDER_CAPABILITY_SCRIPT" "$builder_path" 2>/dev/null) ||
        capability_status=$?
    if [ "$capability_status" -ne 0 ]; then
        echo -e "${RED}ERROR: builder capability probe failed with status $capability_status: $builder_path${NC}" >&2
        exit 126
    fi
    [ "$capability" = "advertised-jobs" ]
}

tier_builder_supports_no_fork() {
    local builder_path=$1
    local capability
    local capability_status=0
    capability=$(env -u LD_PRELOAD -u SEEN_FORK_SERIALIZER_TARGET \
        -u SEEN_FORK_SERIALIZER_ROOT_PID \
        bash "$BUILDER_CAPABILITY_SCRIPT" "$builder_path" 2>/dev/null) ||
        capability_status=$?
    if [ "$capability_status" -ne 0 ]; then
        echo -e "${RED}ERROR: builder capability probe failed with status $capability_status: $builder_path${NC}" >&2
        exit 126
    fi
    [ "$capability" = "advertised-no-fork" ]
}

run_tier_prebuild_gates_if_needed() {
    if [ "$REBUILD_TIER" != "verify" ]; then
        return 0
    fi
    if [ "${SEEN_SKIP_PREBUILD_GATES:-0}" = "1" ]; then
        echo -e "${YELLOW}Prebuild gates skipped by SEEN_SKIP_PREBUILD_GATES=1.${NC}"
        return 0
    fi

    echo "Running verify-tier prebuild gates..."
    if run_guarded_command_to_log_with_failure_watch "prebuild gates" 900 "$MAIN_COMPILER_VMEM_KB" \
        /tmp/safe_rebuild_verify_prebuild_gates.log \
        env SEEN_PACKAGE_CLIENT="$SOURCE_PACKAGE_CLIENT" \
            bash "$SCRIPT_DIR/seen_prebuild_gates.sh"; then
        return 0
    fi
    echo -e "${RED}ERROR: verify-tier prebuild gates failed.${NC}"
    tail_log_if_exists /tmp/safe_rebuild_verify_prebuild_gates.log 30
    return 1
}

run_stage1_acceptance_checks() {
    local compiler_path=$1
    local acceptance_tier=$2
    local acceptance_log="/tmp/safe_rebuild_${acceptance_tier}_stage1_acceptance.log"
    local acceptance_timeout=1800

    if [ "$acceptance_tier" = "verify" ]; then
        acceptance_timeout=7200
    fi

    echo "Running ${acceptance_tier}-tier Stage-1 acceptance checks against $compiler_path..."
    if run_guarded_command_to_log_with_failure_watch \
        "${acceptance_tier} Stage-1 acceptance" "$acceptance_timeout" \
        "$MAIN_COMPILER_VMEM_KB" "$acceptance_log" \
        env \
            SEEN_PACKAGE_CLIENT="$SOURCE_PACKAGE_CLIENT" \
            SEEN_COMPILER_SOURCE_ROOT="$REPO_ROOT" \
            SEEN_LOW_MEMORY=1 \
            SEEN_MAIN_VMEM_KB="$MAIN_COMPILER_VMEM_KB" \
            SEEN_OPT_VMEM_KB="$OPT_VMEM_KB" \
            SEEN_MEMORY_LIMIT_BYTES="$SEEN_MEMORY_LIMIT_BYTES" \
            SEEN_PROJECT_ARTIFACT_WRAPPER=1 \
            "$SCRIPT_DIR/seen_stage1_acceptance.sh" \
                --tier "$acceptance_tier" --compiler "$compiler_path"; then

        return 0
    fi

    echo -e "${RED}ERROR: ${acceptance_tier}-tier Stage-1 acceptance checks failed.${NC}"
    tail_log_if_exists "$acceptance_log" 40
    return 1
}

run_tier_targeted_checks() {
    local compiler_path=$1

    run_stage1_acceptance_checks "$compiler_path" "$REBUILD_TIER" || return 1

    if [ "$REBUILD_TIER" != "verify" ]; then
        return 0
    fi

    if [ -f "$REPO_ROOT/compiler_seen/tests/dead_code_warnings.seen" ]; then
        if ! run_guarded_command_to_log_with_failure_watch "verify dead-code test check" 300 "$MAIN_COMPILER_VMEM_KB" \
            /tmp/safe_rebuild_verify_dead_code.log \
            env -u SEEN_FORK_SERIALIZER_ROOT_PID \
            SEEN_PACKAGE_CLIENT="$SOURCE_PACKAGE_CLIENT" \
            SEEN_COMPILER_SOURCE_ROOT="$REPO_ROOT" \
            LD_PRELOAD="$FORK_SERIALIZER_SO" \
            SEEN_FORK_SERIALIZER_TARGET="$compiler_path" \
            "$compiler_path" check "$REPO_ROOT/compiler_seen/tests/dead_code_warnings.seen"; then
            echo -e "${RED}ERROR: verify-tier dead-code test check failed.${NC}"
            tail_log_if_exists /tmp/safe_rebuild_verify_dead_code.log 30
            return 1
        fi
    fi

    return 0
}

prepare_package_client() {
    local expected_version helper handshake expected_handshake
    expected_version=$(awk -F'"' '/^version = / { print $2; exit }' "$REPO_ROOT/Seen.toml")
    if [ -z "$expected_version" ]; then
        echo -e "${RED}ERROR: could not read the Seen version for package-client coupling.${NC}" >&2
        return 1
    fi

    if [ -n "${SEEN_PACKAGE_CLIENT:-}" ]; then
        helper="$SEEN_PACKAGE_CLIENT"
        if [ ! -f "$helper" ] || [ ! -x "$helper" ] || [ -L "$helper" ]; then
            echo -e "${RED}ERROR: SEEN_PACKAGE_CLIENT is not a regular executable: $helper${NC}" >&2
            return 1
        fi
    else
        mkdir -p "$(dirname "$PACKAGE_CLIENT_BUILD_OUTPUT")"
        run_guarded_command "package client build" 600 "$OPT_VMEM_KB" \
            "$SCRIPT_DIR/build_package_client.sh" \
                --version "$expected_version" \
                --output "$PACKAGE_CLIENT_BUILD_OUTPUT" || return 1
        helper="$PACKAGE_CLIENT_BUILD_OUTPUT"
    fi

    helper="$(cd "$(dirname "$helper")" && pwd -P)/$(basename "$helper")"
    if ! handshake=$("$helper" --expect-version "$expected_version" \
        version --machine 2>&1); then

        echo -e "${RED}ERROR: package-client version handshake failed for Seen $expected_version.${NC}" >&2
        return 1
    fi
    expected_handshake=$(printf 'protocol=SEENPKG1\nversion=%s' "$expected_version")
    if [ "$handshake" != "$expected_handshake" ]; then
        echo -e "${RED}ERROR: package-client returned a malformed Seen $expected_version handshake.${NC}" >&2
        return 1
    fi

    # Keep the source-version helper distinct from candidate state. Quick and
    # verify pass it only to an exact-version builder; full-tier legacy/frozen
    # builders run with SEEN_PACKAGE_CLIENT unset.
    SOURCE_PACKAGE_CLIENT_VERSION="$expected_version"
    SOURCE_PACKAGE_CLIENT="$helper"
    PACKAGE_CLIENT_BUILD_OUTPUT="$SOURCE_PACKAGE_CLIENT"
    unset SEEN_PACKAGE_CLIENT
    echo "Source package client: $SOURCE_PACKAGE_CLIENT"
}

install_tier_verified_compiler() {
    local compiler_path=$1

    echo ""
    echo "Installing verified compiler..."
    safe_rebuild_install_checkout_file "$compiler_path" \
        compiler_seen/target/seen || return 1
    safe_rebuild_install_checkout_file "$compiler_path" \
        target/release/seen || return 1
    safe_rebuild_install_checkout_file "$PACKAGE_CLIENT_BUILD_OUTPUT" \
        compiler_seen/target/seen-pkg || return 1
    safe_rebuild_install_checkout_file "$PACKAGE_CLIENT_BUILD_OUTPUT" \
        target/release/seen-pkg || return 1
}

run_tiered_rebuild() {
    local output_path final_output_path label compile_log candidate source_root builder_for_root
    local compile_status compile_flags

    label="$REBUILD_TIER"
    if [ "$REBUILD_TIER" = "quick" ]; then
        output_path="/tmp/seen_quick_rebuild"
        final_output_path="$REPO_ROOT/compiler_seen/target/seen-dev"
        compile_log="/tmp/safe_rebuild_quick.log"
    else
        output_path="/tmp/seen_verify_rebuild"
        final_output_path="$output_path"
        compile_log="/tmp/safe_rebuild_verify.log"
    fi

    if [ "$CLEAN_CACHE" = "1" ]; then
        clean_rebuild_caches "$REBUILD_TIER --clean-cache"
    fi
    run_tier_prebuild_gates_if_needed || return 1

    echo ""
    echo "Tiered rebuild: $REBUILD_TIER"
    echo "  SEEN_JOBS=$SEEN_JOBS SEEN_OPT_JOBS=$SEEN_OPT_JOBS"
    echo "  Output: $final_output_path"

    rm -f "$output_path"
    mkdir -p "$(dirname "$output_path")"

    candidate=$(select_tier_builder) || {
        echo -e "${RED}ERROR: no exact-version quick/verify builder passed selection.${NC}" >&2
        return 1
    }
    [ -f "$candidate" ] && [ -x "$candidate" ] && [ ! -L "$candidate" ] || {
        echo -e "${RED}ERROR: selected tier builder became unsafe: $candidate${NC}" >&2
        return 1
    }
    if tier_builder_requires_bootstrap_preflight "$candidate"; then
        echo -e "${RED}ERROR: quick/verify may not select a frozen bootstrap builder.${NC}" >&2
        return 1
    fi
    source_root=$(tier_source_root_for_builder "$candidate")
    builder_for_root=$(tier_builder_path_for_source_root "$candidate" "$source_root")
    [ -f "$builder_for_root" ] && [ -x "$builder_for_root" ] &&
        [ ! -L "$builder_for_root" ] || {
        echo -e "${RED}ERROR: selected tier builder is unavailable in its source root.${NC}" >&2
        return 1
    }
    if ! compiler_reports_checkout_version "$builder_for_root"; then
        echo -e "${RED}ERROR: selected tier builder lost exact checkout-version identity.${NC}" >&2
        return 1
    fi
    if ! compiler_serializer_applicable "$builder_for_root"; then
        echo -e "${RED}ERROR: selected tier builder failed serializer applicability checks.${NC}" >&2
        return 1
    fi

    echo "  Selected builder: $candidate"
    compile_status=0
    compile_flags=(--fast)
    if [ -n "${RELEASE_TARGET_CPU_FLAG:-}" ]; then
        compile_flags+=("$RELEASE_TARGET_CPU_FLAG")
    fi
    if tier_builder_supports_jobs "$builder_for_root"; then
        compile_flags+=(--jobs "$SEEN_JOBS" --opt-jobs "$SEEN_OPT_JOBS")
    elif tier_builder_supports_no_fork "$builder_for_root"; then
        compile_flags+=(--no-fork)
    else
        echo -e "${YELLOW}    Builder advertises no worker controls; direct-child serialization and the hard cgroup remain mandatory.${NC}"
    fi

    run_guarded_command_to_log_with_failure_watch "$label compile" "$TIER_TIMEOUT_SECS" "$MAIN_COMPILER_VMEM_KB" "$compile_log" \
        bash -c 'cd "$1" || exit 1; shift; exec "$@"' bash "$source_root" \
        env \
            SEEN_PACKAGE_CLIENT="$SOURCE_PACKAGE_CLIENT" \
            SEEN_COMPILER_SOURCE_ROOT="$source_root" \
            SEEN_LOW_MEMORY="${SEEN_LOW_MEMORY:-0}" \
            SEEN_MAIN_VMEM_KB="$MAIN_COMPILER_VMEM_KB" \
            SEEN_OPT_VMEM_KB="$OPT_VMEM_KB" \
            SEEN_MEMORY_LIMIT_BYTES="${SEEN_MEMORY_LIMIT_BYTES:-}" \
            SEEN_JOBS="$SEEN_JOBS" \
            SEEN_OPT_JOBS="$SEEN_OPT_JOBS" \
            LD_PRELOAD="$FORK_SERIALIZER_SO" \
            SEEN_FORK_SERIALIZER_TARGET="$builder_for_root" \
            SEEN_FORK_SERIALIZER_ROOT_PID= \
            "$builder_for_root" compile "$COMPILER_SOURCE" "$output_path" \
            "${compile_flags[@]}" || compile_status=$?

    if grep -qE 'Fatal Lexer Error|Fatal Parser Error' "$compile_log" 2>/dev/null; then
        echo -e "${YELLOW}  Builder emitted a fatal frontend diagnostic; rejecting its output.${NC}"
        compile_status=1
    fi

    if [ "$compile_status" -ne 0 ]; then
        echo -e "${RED}ERROR: selected $REBUILD_TIER builder failed (exit=$compile_status); legacy fallback is forbidden.${NC}" >&2
        tail_log_if_exists "$compile_log" 10
        rm -f "$output_path"
        return "$compile_status"
    fi

    if ! compiler_reports_checkout_version "$output_path"; then
        echo -e "${RED}ERROR: selected builder produced a compiler with the wrong version identity; fallback is forbidden.${NC}" >&2
        rm -f "$output_path"
        return 1
    fi
    if ! smoke_test_compiler "$output_path" "$REBUILD_TIER compiler" "$REBUILD_TIER"; then
        echo -e "${RED}ERROR: compiler from the selected builder failed smoke; legacy fallback is forbidden.${NC}" >&2
        rm -f "$output_path"
        return 1
    fi

    run_tier_targeted_checks "$output_path" || return 1
    if [ "$REBUILD_TIER" = "verify" ]; then
        install_tier_verified_compiler "$output_path" || return 1
    else
        safe_rebuild_install_checkout_file "$output_path" \
            compiler_seen/target/seen-dev || return 1
        safe_rebuild_install_checkout_file "$PACKAGE_CLIENT_BUILD_OUTPUT" \
            compiler_seen/target/seen-pkg || return 1
    fi
    echo -e "${GREEN}${REBUILD_TIER} rebuild complete.${NC}"
    if [ "$REBUILD_TIER" = "quick" ]; then
        echo "Developer compiler: compiler_seen/target/seen-dev"
    else
        echo "Production compiler updated: compiler_seen/target/seen"
        echo "Also installed to: target/release/seen"
    fi
    return 0
}

recover_with_preserved_production_compiler() {
    if [ ! -x "$PRESERVED_PROD_BUILDER" ]; then
        return 1
    fi

    echo ""
    echo "Recovery: trying preserved production compiler..."
    if ! smoke_test_compiler "$PRESERVED_PROD_BUILDER" "Preserved production compiler" "preserved_prod"; then
        echo -e "${YELLOW}Preserved production compiler failed smoke; skipping recovery rebuild.${NC}"
        return 1
    fi

    cleanup_smoke_build_state
    rm -f "$STAGE3_RECOVERY"

    local recovery_source_root="$REPO_ROOT"
    local source_mode="${SEEN_EXISTING_BUILDER_SOURCE_ROOT:-auto}"
    if [ "${SEEN_EXISTING_BUILDER_USE_BOOTSTRAP_OVERLAY:-0}" = "1" ]; then
        source_mode="overlay"
    fi
    case "$source_mode" in
        overlay)
            recovery_source_root="$BOOTSTRAP_SOURCE_ROOT"
            ;;
        real)
            recovery_source_root="$REPO_ROOT"
            ;;
        auto)
            case "$(basename "$PRESERVED_PROD_BUILDER")" in
                seen_frozen*|stage1_frozen*) recovery_source_root="$BOOTSTRAP_SOURCE_ROOT" ;;
            esac
            ;;
        *)
            echo -e "${RED}ERROR: SEEN_EXISTING_BUILDER_SOURCE_ROOT must be auto, real, or overlay.${NC}" >&2
            return 1
            ;;
    esac
    local recovery_builder_path="$PRESERVED_PROD_BUILDER"
    case "$recovery_builder_path" in
        "$REPO_ROOT"/*)
            if [ "$recovery_source_root" = "$BOOTSTRAP_SOURCE_ROOT" ]; then
                recovery_builder_path="$BOOTSTRAP_SOURCE_ROOT/${recovery_builder_path#$REPO_ROOT/}"
            fi
            ;;
    esac

    local recovery_exit=0
    local recovery_marker=""
    local recovery_concurrency_flags=()
    local recovery_package_env=(env -u SEEN_PACKAGE_CLIENT)
    if ! compiler_serializer_applicable "$recovery_builder_path"; then
        echo -e "${YELLOW}Preserved production compiler failed serializer applicability checks.${NC}"
        return 1
    fi
    if tier_builder_supports_jobs "$recovery_builder_path"; then
        recovery_concurrency_flags+=(--jobs "$SEEN_JOBS" --opt-jobs "$SEEN_OPT_JOBS")
    elif tier_builder_supports_no_fork "$recovery_builder_path"; then
        recovery_concurrency_flags+=(--no-fork)
    else
        echo -e "${YELLOW}Preserved production compiler advertises no worker controls; direct-child serialization and the hard cgroup remain mandatory.${NC}"
    fi
    if compiler_reports_checkout_version "$recovery_builder_path"; then
        recovery_package_env+=("SEEN_PACKAGE_CLIENT=$SOURCE_PACKAGE_CLIENT")
    fi
    recovery_marker=$(mktemp -d /tmp/seen_preserved_recovery_marker.XXXXXX 2>/dev/null || true)
    run_guarded_command_to_log "preserved compiler recovery" "$RECOVERY_TIMEOUT_SECS" "$MAIN_COMPILER_VMEM_KB" /tmp/safe_rebuild_stage3_recovery.log \
        bash -c 'cd "$1" || exit 1; shift; exec "$@"' bash "$recovery_source_root" \
        "${recovery_package_env[@]}" PATH="$OPT_WRAPPER_DIR:$PATH" \
            SEEN_COMPILER_SOURCE_ROOT="$recovery_source_root" \
            SEEN_LOW_MEMORY="${SEEN_LOW_MEMORY:-0}" \
            SEEN_MAIN_VMEM_KB="$MAIN_COMPILER_VMEM_KB" \
            SEEN_OPT_VMEM_KB="$OPT_VMEM_KB" \
            SEEN_MEMORY_LIMIT_BYTES="${SEEN_MEMORY_LIMIT_BYTES:-}" \
            LD_PRELOAD="$FORK_SERIALIZER_SO" \
            SEEN_FORK_SERIALIZER_TARGET="$recovery_builder_path" \
            SEEN_FORK_SERIALIZER_ROOT_PID= \
            "$recovery_builder_path" compile "$COMPILER_SOURCE" "$STAGE3_RECOVERY" \
            --fast --no-cache "${recovery_concurrency_flags[@]}" $RELEASE_TARGET_CPU_FLAG || recovery_exit=$?
    if [ "$recovery_exit" -eq 0 ]; then
        rm -rf "$recovery_marker"
        echo -e "${GREEN}Recovery rebuild succeeded.${NC}"
        echo ""
        echo "Recovery smoke: checking hello-world..."
        if smoke_test_compiler "$STAGE3_RECOVERY" "Recovered stage3" "stage3_recovery"; then
            VERIFIED="$STAGE3_RECOVERY"
            return 0
        fi
        echo -e "${YELLOW}Recovery rebuild produced a compiler that failed smoke.${NC}"
        return 1
    fi

    echo -e "${YELLOW}Recovery rebuild failed or timed out (exit=$recovery_exit).${NC}"
    if [ "$recovery_exit" != "124" ]; then
        tail_log_if_exists /tmp/safe_rebuild_stage3_recovery.log 10
    fi
    local preserved_expected_modules=0
    local preserved_ll_dir=""
    preserved_expected_modules=$(extract_expected_module_count /tmp/safe_rebuild_stage3_recovery.log)
    if is_positive_integer "$preserved_expected_modules" && [ "$preserved_expected_modules" -gt 0 ]; then
        preserved_ll_dir=$(find_latest_compile_ll_dir_with_count "$preserved_expected_modules" "$recovery_marker")
        if [ -n "$preserved_ll_dir" ]; then
            LL_COUNT="$preserved_expected_modules"
            LL_SOURCE="preserved compiler recovery"
            LL_RECOVERY_SOURCE_DIR="$preserved_ll_dir"
            EXPECTED_STAGE2_MODULES="$preserved_expected_modules"
            echo -e "${YELLOW}Preserved compiler left a complete $LL_COUNT/$EXPECTED_STAGE2_MODULES .ll set at $LL_RECOVERY_SOURCE_DIR; falling back to direct IR recovery.${NC}"
        fi
    fi
    rm -rf "$recovery_marker"
    return 1
}

link_recovered_compiler() {
    local output_path=$1
    local recovery_dir=$2
    local label=$3

    local obj_count
    obj_count=$(count_module_objects "$recovery_dir")
    echo "  ${label}: $obj_count recovered module objects ready."

    local rt_dir
    rt_dir="$(cd "$SCRIPT_DIR/.." && pwd)/seen_runtime"
    if [ ! -f "$rt_dir/seen_runtime.o" ] || [ "$rt_dir/seen_runtime.c" -nt "$rt_dir/seen_runtime.o" ]; then
        echo "  Pre-compiling runtime..."
        run_guarded_command "${label} runtime seen_runtime.c" 300 "$OPT_VMEM_KB" \
            clang -O3 -flto=thin "$RELEASE_CLANG_MARCH_FLAG" -ffunction-sections -fdata-sections -pthread \
            -c -I "$rt_dir" "$rt_dir/seen_runtime.c" -o "$rt_dir/seen_runtime.o" 2>/dev/null || true
    fi
    if [ -f "$rt_dir/seen_region.c" ]; then
        if [ ! -f "$rt_dir/seen_region.o" ] || [ "$rt_dir/seen_region.c" -nt "$rt_dir/seen_region.o" ]; then
            run_guarded_command "${label} runtime seen_region.c" 300 "$OPT_VMEM_KB" \
                clang -O3 -flto=thin "$RELEASE_CLANG_MARCH_FLAG" -ffunction-sections -fdata-sections \
                -c -I "$rt_dir" "$rt_dir/seen_region.c" -o "$rt_dir/seen_region.o" 2>/dev/null || true
        fi
    fi
    if [ -f "$rt_dir/seen_gpu.c" ]; then
        if [ ! -f "$rt_dir/seen_gpu.o" ] || [ "$rt_dir/seen_gpu.c" -nt "$rt_dir/seen_gpu.o" ]; then
            run_guarded_command "${label} runtime seen_gpu.c" 300 "$OPT_VMEM_KB" \
                clang -O3 -flto=thin "$RELEASE_CLANG_MARCH_FLAG" -ffunction-sections -fdata-sections \
                -c -I "$rt_dir" "$rt_dir/seen_gpu.c" -o "$rt_dir/seen_gpu.o" 2>/dev/null || true
        fi
    fi

    local link_objs=""
    local obj
    for obj in "$recovery_dir"/seen_module_*.o; do
        link_objs="$link_objs $obj"
    done

    local rt_objs="$rt_dir/seen_runtime.o"
    [ -f "$rt_dir/seen_region.o" ] && rt_objs="$rt_objs $rt_dir/seen_region.o"
    [ -f "$rt_dir/seen_gpu.o" ] && rt_objs="$rt_objs $rt_dir/seen_gpu.o"

    local link_libs="-lm -lpthread"
    [ -f "$rt_dir/seen_gpu.o" ] && pkg-config --exists vulkan 2>/dev/null && link_libs="$link_libs -lvulkan"

    echo "  Linking $obj_count modules..."
    if run_guarded_command "${label} recovery link" 0 "$OPT_VMEM_KB" clang -O1 -fuse-ld=lld \
        -Wl,--allow-multiple-definition \
        "$RELEASE_CLANG_MARCH_FLAG" -Wl,--gc-sections -Wno-unused-command-line-argument \
        $link_objs $rt_objs -o "$output_path" $link_libs 2>/tmp/safe_rebuild_link.log; then
        echo -e "${GREEN}${label} recovery link succeeded ($(wc -c < "$output_path" | tr -d ' ') bytes).${NC}"
        return 0
    fi

    echo -e "${RED}ERROR: ${label} recovery link failed.${NC}"
    grep -E 'undefined|error' /tmp/safe_rebuild_link.log | head -10
    return 1
}

recover_complete_ll_set_to_compiler() {
    local expected_modules=$1
    local marker_dir=$2
    local output_path=$3
    local label=$4

    if [ "$IR_RECOVERY_DISABLED" = "1" ]; then
        echo -e "${RED}ERROR: ${label} failed and SEEN_DISABLE_IR_RECOVERY=1 forbids direct IR recovery.${NC}"
        return 1
    fi

    if ! is_positive_integer "$expected_modules" || [ "$expected_modules" -le 0 ]; then
        return 1
    fi

    local ll_dir
    ll_dir=$(find_latest_compile_ll_dir_with_count "$expected_modules" "$marker_dir")
    if [ -z "$ll_dir" ]; then
        return 1
    fi

    echo -e "${YELLOW}${label} left a complete $expected_modules/$expected_modules unmodified .ll set at $ll_dir; using direct object recovery.${NC}"

    local recovery_exit=0
    local recovery_log="/tmp/seen_${label//[^A-Za-z0-9_]/_}_recovery_$$.log"
    set +e
    run_guarded_command "${label} unmodified IR recovery" "$RECOVERY_TIMEOUT_SECS" "$OPT_VMEM_KB" \
        bash "$SCRIPT_DIR/recovery_opt.sh" "$OPT_WRAPPER_DIR" "$SCRIPT_DIR" "$ll_dir" --skip-fixups 2>&1 | tee "$recovery_log"
    recovery_exit=${PIPESTATUS[0]}
    set -e

    local recovery_output
    recovery_output=$(cat "$recovery_log" 2>/dev/null || true)
    rm -f "$recovery_log"

    if [ "$recovery_exit" -ne 0 ]; then
        echo -e "${RED}ERROR: ${label} IR recovery failed.${NC}"
        return 1
    fi

    local recovery_dir
    recovery_dir=$(echo "$recovery_output" | grep '^RECOVERY_DIR=' | tail -1 | cut -d= -f2)
    if [ -z "$recovery_dir" ] || [ ! -d "$recovery_dir" ]; then
        echo -e "${RED}ERROR: ${label} IR recovery did not return an output directory.${NC}"
        return 1
    fi

    local obj_count
    obj_count=$(count_module_objects "$recovery_dir")
    if [ "$obj_count" -ne "$expected_modules" ]; then
        local missing_modules
        missing_modules=$(list_modules_missing_objects "$recovery_dir")
        echo -e "${RED}ERROR: ${label} IR recovery produced only $obj_count/$expected_modules objects.${NC}"
        if [ -n "$missing_modules" ]; then
            echo "Missing objects:$missing_modules"
        fi
        rm -rf "$recovery_dir"
        return 1
    fi

    if link_recovered_compiler "$output_path" "$recovery_dir" "$label"; then
        rm -rf "$recovery_dir"
        return 0
    fi

    rm -rf "$recovery_dir"
    return 1
}

recover_with_existing_stage_builder() {
    local builder_path=$1
    local builder_name
    local builder_slug
    local builder_log
    local recovery_concurrency_flags=()
    local recovery_package_env=(env -u SEEN_PACKAGE_CLIENT)

    [ -x "$builder_path" ] || return 1
    builder_name=$(basename "$builder_path")
    builder_slug=$(printf "%s" "$builder_name" | tr -c 'A-Za-z0-9_' '_')
    builder_log="/tmp/safe_rebuild_existing_stage_${builder_slug}.log"

    if tier_builder_supports_jobs "$builder_path"; then
        recovery_concurrency_flags+=(--jobs "$SEEN_JOBS" --opt-jobs "$SEEN_OPT_JOBS")
    elif tier_builder_supports_no_fork "$builder_path"; then
        recovery_concurrency_flags+=(--no-fork)
    else
        echo -e "${YELLOW}Existing stage builder advertises no worker controls; direct-child serialization and the hard cgroup remain mandatory.${NC}"
    fi

    echo ""
    echo "Recovery: trying existing stage builder $builder_path..."
    if ! smoke_test_compiler "$builder_path" "Existing stage builder $builder_name" "existing_${builder_slug}"; then
        echo -e "${YELLOW}Existing stage builder $builder_name failed smoke; trying next recovery builder.${NC}"
        return 1
    fi

    cleanup_smoke_build_state
    rm -f "$STAGE3_RECOVERY"

    local recovery_source_root="$REPO_ROOT"
    local source_mode="${SEEN_EXISTING_BUILDER_SOURCE_ROOT:-auto}"
    if [ "${SEEN_EXISTING_BUILDER_USE_BOOTSTRAP_OVERLAY:-0}" = "1" ]; then
        source_mode="overlay"
    fi
    case "$source_mode" in
        overlay)
            recovery_source_root="$BOOTSTRAP_SOURCE_ROOT"
            ;;
        real)
            recovery_source_root="$REPO_ROOT"
            ;;
        auto)
            case "$builder_name" in
                seen_frozen*|stage1_frozen*) recovery_source_root="$BOOTSTRAP_SOURCE_ROOT" ;;
            esac
            ;;
        *)
            echo -e "${RED}ERROR: SEEN_EXISTING_BUILDER_SOURCE_ROOT must be auto, real, or overlay.${NC}" >&2
            return 1
            ;;
    esac
    local recovery_builder_path="$builder_path"
    case "$recovery_builder_path" in
        "$REPO_ROOT"/*)
            if [ "$recovery_source_root" = "$BOOTSTRAP_SOURCE_ROOT" ]; then
                recovery_builder_path="$BOOTSTRAP_SOURCE_ROOT/${recovery_builder_path#$REPO_ROOT/}"
            fi
            ;;
    esac

    local recovery_exit=0
    if ! compiler_serializer_applicable "$recovery_builder_path"; then
        echo -e "${YELLOW}Existing stage builder failed serializer applicability checks.${NC}"
        return 1
    fi
    if compiler_reports_checkout_version "$recovery_builder_path"; then
        recovery_package_env+=("SEEN_PACKAGE_CLIENT=$SOURCE_PACKAGE_CLIENT")
    fi
    run_guarded_command_to_log "existing stage builder $builder_name recovery" "$RECOVERY_TIMEOUT_SECS" "$MAIN_COMPILER_VMEM_KB" "$builder_log" \
        bash -c 'cd "$1" || exit 1; shift; exec "$@"' bash "$recovery_source_root" \
        "${recovery_package_env[@]}" PATH="$OPT_WRAPPER_DIR:$PATH" \
            SEEN_COMPILER_SOURCE_ROOT="$recovery_source_root" \
            SEEN_LOW_MEMORY="${SEEN_LOW_MEMORY:-0}" \
            SEEN_MAIN_VMEM_KB="$MAIN_COMPILER_VMEM_KB" \
            SEEN_OPT_VMEM_KB="$OPT_VMEM_KB" \
            SEEN_MEMORY_LIMIT_BYTES="${SEEN_MEMORY_LIMIT_BYTES:-}" \
            LD_PRELOAD="$FORK_SERIALIZER_SO" \
            SEEN_FORK_SERIALIZER_TARGET="$recovery_builder_path" \
            SEEN_FORK_SERIALIZER_ROOT_PID= \
            "$recovery_builder_path" compile "$COMPILER_SOURCE" "$STAGE3_RECOVERY" \
            --fast --no-cache "${recovery_concurrency_flags[@]}" $RELEASE_TARGET_CPU_FLAG || recovery_exit=$?

    if [ "$recovery_exit" -eq 0 ]; then
        echo -e "${GREEN}Existing stage builder $builder_name rebuilt the compiler.${NC}"
        echo ""
        echo "Recovery smoke: checking hello-world..."
        if smoke_test_compiler "$STAGE3_RECOVERY" "Recovered compiler from $builder_name" "recovered_${builder_slug}"; then
            VERIFIED="$STAGE3_RECOVERY"
            return 0
        fi
        echo -e "${YELLOW}Recovered compiler from $builder_name failed smoke; trying next recovery builder.${NC}"
        return 1
    fi

    echo -e "${YELLOW}Existing stage builder $builder_name failed or timed out (exit=$recovery_exit).${NC}"
    if [ "$recovery_exit" != "124" ]; then
        tail_log_if_exists "$builder_log" 10
    fi
    return 1
}

recover_with_existing_stage_builders() {
    local candidate
    local candidates=()

    if [ -n "${SEEN_STAGE_BUILDER:-}" ]; then
        candidates+=("$SEEN_STAGE_BUILDER")
    fi
    candidates+=(
        "$REPO_ROOT/stage2_head"
        "$REPO_ROOT/stage3_recovery_head"
        "$REPO_ROOT/stage3_head"
        "$REPO_ROOT/compiler_seen/target/seen_native_snapshot"
        "$REPO_ROOT/compiler_seen/target/seen_frozen6"
    )

    for candidate in "${candidates[@]}"; do
        if recover_with_existing_stage_builder "$candidate"; then
            return 0
        fi
        if [ "${SEEN_STAGE_BUILDER_ONLY:-0}" = "1" ] && [ -n "${SEEN_STAGE_BUILDER:-}" ] && [ "$candidate" = "$SEEN_STAGE_BUILDER" ]; then
            return 1
        fi
    done

    return 1
}

preserve_existing_production_compiler >/dev/null 2>&1 || true

# Every compiler produced from 0.10 onward invokes this exact, version-matched
# helper during dependency preparation. Build and verify it before any stage
# can become a builder for the next stage.
prepare_package_client || exit 1

if [ "$REBUILD_TIER" != "full" ]; then
    run_tiered_rebuild
    exit $?
fi

ensure_bootstrap_preflight || exit 1

clean_rebuild_caches "full tier cold verification"

if [ "${SEEN_SKIP_PREBUILD_GATES:-0}" != "1" ]; then
    echo "Running prebuild gates..."
    if ! run_guarded_command_to_log_with_failure_watch "prebuild gates" 900 "$MAIN_COMPILER_VMEM_KB" \
        /tmp/safe_rebuild_prebuild_gates.log \
        env SEEN_PACKAGE_CLIENT="$SOURCE_PACKAGE_CLIENT" \
            SEEN_DEFER_SELFHOSTED_ABI_SMOKE=1 \
            bash "$SCRIPT_DIR/seen_prebuild_gates.sh"; then
        echo -e "${RED}ERROR: prebuild gates failed.${NC}"
        tail_log_if_exists /tmp/safe_rebuild_prebuild_gates.log 30
        exit 1
    fi
else
    echo -e "${YELLOW}Prebuild gates skipped by SEEN_SKIP_PREBUILD_GATES=1.${NC}"
fi

# Kill any leftover compilation processes that might write to /tmp/seen_module_*
# and interfere with this build (race condition causes duplicate symbols)
kill_scope_matching_processes "seen compile"
kill_scope_matching_processes "seen build"
sleep 1

# Clean up any previous test files and cache
rm -f "$STAGE2" "$STAGE3"

# --- Opt wrapper setup (platform-specific) ---

if [ "$HOST_OS" = "Darwin" ]; then
    # macOS: expose the historical ABI adapter only to the exact frozen
    # Stage-1 invocation. Current/production compilers exec the real optimizer.
    OPT_WRAPPER_DIR=$(mktemp -d /tmp/seen_opt_wrapper.XXXXXX)
    if [ -d "/opt/homebrew/opt/llvm/bin" ]; then
        LLVM_BIN="/opt/homebrew/opt/llvm/bin"
    elif [ -d "/usr/local/opt/llvm/bin" ]; then
        LLVM_BIN="/usr/local/opt/llvm/bin"
    else
        LLVM_BIN=""
    fi
    PYTHON3_PATH=""
    for p in /opt/homebrew/bin/python3 /usr/local/bin/python3 /usr/bin/python3; do
        if [ -x "$p" ]; then PYTHON3_PATH="$p"; break; fi
    done
    [ -z "$PYTHON3_PATH" ] && PYTHON3_PATH=$(which python3 2>/dev/null || echo "python3")
    REAL_OPT=""
    for candidate in "$LLVM_BIN/opt" /opt/homebrew/opt/llvm/bin/opt \
        /usr/local/opt/llvm/bin/opt /usr/bin/opt; do

        if [ -x "$candidate" ]; then
            REAL_OPT="$candidate"
            break
        fi
    done
    [ -z "$REAL_OPT" ] && REAL_OPT=$(command -v opt 2>/dev/null || true)
    if [ -z "$REAL_OPT" ] || [ ! -x "$REAL_OPT" ]; then
        echo -e "${RED}ERROR: real macOS LLVM opt binary not found.${NC}" >&2
        exit 1
    fi
    cp bootstrap/macos_opt_wrapper.py "$OPT_WRAPPER_DIR/macos_opt_wrapper_impl.py"
    cat > "$OPT_WRAPPER_DIR/opt" << WRAPPER_EOF
#!/bin/sh
export PATH="$LLVM_BIN:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:\$PATH"
if [ "\${SEEN_FROZEN_IR_COMPAT:-0}" != "1" ]; then
    exec "$REAL_OPT" "\$@"
fi
exec "$PYTHON3_PATH" "$OPT_WRAPPER_DIR/macos_opt_wrapper_impl.py" "\$@"
WRAPPER_EOF
    chmod +x "$OPT_WRAPPER_DIR/opt"
    export PATH="$OPT_WRAPPER_DIR:$LLVM_BIN:$PATH"
    echo "macOS: frozen Stage-1 compatibility wrapper ready (python3=$PYTHON3_PATH)"
else
    # Linux: the immutable frozen Stage-1 compiler needs an explicit,
    # bootstrap-only compatibility adapter. Every current/production compiler
    # reaches the real optimizer unchanged unless the exact frozen invocation
    # opts in with SEEN_FROZEN_IR_COMPAT=1.
    REAL_OPT=$(command -v opt)
    OPT_WRAPPER_DIR="/tmp/seen_opt_override"
    mkdir -p "$OPT_WRAPPER_DIR"
cat > "$OPT_WRAPPER_DIR/opt" << WRAPPER_EOF
#!/bin/bash
# Wrapper: deduplicate declare statements in .ll files before invoking real opt.
# The frozen compiler emits extern __-prefixed functions twice (once from ir_declarations
# with nounwind, once from extern handler with possibly different types). We do a two-pass
# approach: first collect all declared function names, then on second pass keep only the
# LAST declaration for each function (which matches call site types).
ARGS=("\$@")
if [ "\${SEEN_LOW_MEMORY:-0}" = "1" ] && [ -n "\${SEEN_OPT_VMEM_KB:-}" ]; then
    # Cap the whole wrapper so fix_ir.py and llvm-as stay within the low-memory budget too.
    if ! ulimit -S -v "\$SEEN_OPT_VMEM_KB" 2>/dev/null; then
        echo "RESOURCE STOP: could not apply optimizer virtual-memory cap (\${SEEN_OPT_VMEM_KB} KiB)" >&2
        exit 126
    fi
    ACTIVE_OPT_VMEM=\$(ulimit -S -v 2>/dev/null || true)
    case "\$ACTIVE_OPT_VMEM" in
        ''|*[!0-9]*)
            echo "RESOURCE STOP: could not read back optimizer virtual-memory cap" >&2
            exit 126
            ;;
    esac
    if [ "\$ACTIVE_OPT_VMEM" -gt "\$SEEN_OPT_VMEM_KB" ]; then
        echo "RESOURCE STOP: optimizer virtual-memory cap read-back exceeds \${SEEN_OPT_VMEM_KB} KiB" >&2
        exit 126
    fi
fi
SEEN_OPT_LOCK_HELD=0
acquire_seen_low_memory_opt_lock() {
    if [ "\${SEEN_LOW_MEMORY:-0}" != "1" ] || [ "\$SEEN_OPT_LOCK_HELD" = "1" ]; then
        return
    fi
    if command -v flock >/dev/null 2>&1; then
        exec 9>/tmp/seen_opt_low_memory.lock
        flock 9
    else
        while ! mkdir /tmp/seen_opt_low_memory.lockdir 2>/dev/null; do
            sleep 1
        done
        trap 'rmdir /tmp/seen_opt_low_memory.lockdir 2>/dev/null || true' EXIT
    fi
    SEEN_OPT_LOCK_HELD=1
}
wait_for_stable_ir_file() {
    local file="\$1"
    local prev_size=""
    local cur_size=""
    local stable_count=0
    local attempts=0
    while [ "\$attempts" -lt 50 ]; do
        cur_size=\$(stat -c%s "\$file" 2>/dev/null || stat -f%z "\$file" 2>/dev/null || echo "")
        if [ -n "\$cur_size" ] && [ "\$cur_size" = "\$prev_size" ]; then
            stable_count=\$((stable_count + 1))
            if [ "\$stable_count" -ge 2 ]; then
                return
            fi
        else
            stable_count=0
            prev_size="\$cur_size"
        fi
        attempts=\$((attempts + 1))
        sleep 0.1
    done
}
repair_stale_builder_ir() {
    local file="\$1"
    # Fix C-style void parameters emitted by older builders. LLVM IR spells an
    # empty parameter list as (), and has no void parameter type.
    sed -i 's/, void)/)/g; s/(void, /(/g; s/, void, /, /g; s/(void)\([[:space:]]*\)/()\1/g' "\$file" 2>/dev/null || true

    # Drop impossible dead casts from void-returning calls. Older builders can
    # emit these after void statements, and LLVM rejects void as a value type.
    sed -i '/^[[:space:]]*%[0-9][0-9]* = bitcast void %[0-9][0-9]* to /d' "\$file" 2>/dev/null || true

    # Older builders can also leave invalid aggregate returns in unreachable
    # blocks, for example a ret of an i1 value from a SeenString function.
    # Replace only those verifier-invalid returns with a zero aggregate value.
    python3 - "\$file" <<'PY_RET_FIX' 2>&1 || true
import re
import sys

path = sys.argv[1]
try:
    with open(path, "r", encoding="utf-8") as fh:
        content = fh.read()
except OSError:
    sys.exit(0)

def fix_function_ret_values(function_text):
    i1_values = set(re.findall(r"^\s*(%\d+)\s*=\s*icmp\b", function_text, re.M))
    if not i1_values:
        return function_text

    def fix_ret(match):
        aggregate_type = match.group(1)
        value = match.group(2)
        if value in i1_values:
            return "  ret " + aggregate_type + " zeroinitializer"
        return match.group(0)

    return re.sub(
        r"^\s*ret\s+(%[A-Za-z0-9_.]+)\s+(%\d+)\s*$",
        fix_ret,
        function_text,
        flags=re.M,
    )

parts = []
last = 0
for match in re.finditer(r"^define\b", content, re.M):
    start = match.start()
    if start < last:
        continue
    end_match = re.search(r"^}\s*$", content[start:], re.M)
    if not end_match:
        continue
    end = start + end_match.end()
    parts.append(content[last:start])
    parts.append(fix_function_ret_values(content[start:end]))
    last = end
parts.append(content[last:])
fixed = "".join(parts)
count = 1 if fixed != content else 0
if count > 0 and fixed != content:
    with open(path, "w", encoding="utf-8") as fh:
        fh.write(fixed)
    print("  stale-builder aggregate ret fix applied to " + path, file=sys.stderr)
PY_RET_FIX
}
if [ "\${SEEN_FROZEN_IR_COMPAT:-0}" != "1" ]; then
    acquire_seen_low_memory_opt_lock
    exec "$REAL_OPT" "\$@"
fi
acquire_seen_low_memory_opt_lock
for arg in "\${ARGS[@]}"; do
    if [[ "\$arg" == *.ll && "\$arg" != *.opt.ll && -f "\$arg" ]]; then
        wait_for_stable_ir_file "\$arg"
        awk '
        # Pass 1: count declarations per function name
        NR == FNR {
            if (/^declare /) {
                if (match(\$0, /@([A-Za-z0-9_.]+)/, m)) {
                    count[m[1]]++
                    seen_count[m[1]] = 0
                }
            }
            next
        }
        # Pass 2: for functions with duplicates, skip all but the last
        /^declare / {
            if (match(\$0, /@([A-Za-z0-9_.]+)/, m)) {
                fname = m[1]
                seen_count[fname]++
                if (count[fname] > 1 && seen_count[fname] < count[fname]) next
            }
        }
        { print }
        ' "\$arg" "\$arg" > "\${arg}.dedup" && mv "\${arg}.dedup" "\$arg"

        # Fix byteAt codegen bug: pre-refactoring compiler emits string concat
        # (seen_int_to_string + seen_char_to_str + seen_str_concat_ss) instead of
        # integer add for byteAt() + int expressions. Replace with add i64.
        python3 -c "
import re, sys
with open(sys.argv[1]) as f:
    content = f.read()
pattern = re.compile(
    r'  (%\d+) = call %SeenString @seen_int_to_string\(i64 (%\d+)\)\n'
    r'  (%\d+) = call %SeenString @seen_char_to_str\(i64 (%\d+)\)\n'
    r'  (%\d+) = call %SeenString @seen_str_concat_ss\(%SeenString \1, %SeenString \3\)'
)
def fix(m):
    return f'  {m.group(1)} = add i64 0, 0\n  {m.group(3)} = add i64 0, 0\n  {m.group(5)} = add i64 {m.group(2)}, {m.group(4)}'
new_content, count = pattern.subn(fix, content)
if count > 0:
    with open(sys.argv[1], 'w') as f:
        f.write(new_content)
    print(f'  byteAt fix: patched {count} site(s) in {sys.argv[1]}', file=sys.stderr)
" "\$arg" 2>&1 || true

        # Apply comprehensive IR fixups (declare dedup, type mismatches, SSA, etc.)
        if ! python3 "$SCRIPT_DIR/fix_ir.py" "\$arg" 2>&1; then
            echo "FROZEN IR COMPAT ERROR: fix_ir.py failed for \$arg" >&2
            exit 1
        fi

        # Fix bare 0 as type in declare params (e.g. (i64, 0) → (i64, i64))
        # This is a belt-and-suspenders fix in case fix_ir.py doesn't catch it
        sed -i 's/^\(declare.*(\)\(.*\), 0)/\1\2, i64)/g' "\$arg" 2>/dev/null || true

        repair_stale_builder_ir "\$arg"

        # Fix corrupt declares from string constants leaking into declare generator
        # Stage1 parses @funcName(...) from string constants, producing broken declares
        # with \00 or other garbage. Remove these — the correct declare is already present.
        sed -i '/^declare.*\\\\00/d' "\$arg" 2>/dev/null || true

        # IR call-shape validation: compare direct call sites with declared/defined
        # signatures before opt, so aggregate/scalar ABI drift fails with context.
        if [ "\${SEEN_SKIP_IR_CALL_SHAPE_VERIFY:-0}" != "1" ]; then
            if ! python3 "$SCRIPT_DIR/verify_ir_call_shapes.py" "\$arg" 2>/tmp/seen_call_shape_err.txt; then
                echo "IR CALL SHAPE ERROR: \$arg" >&2
                head -20 /tmp/seen_call_shape_err.txt >&2
                rm -f /tmp/seen_call_shape_err.txt
                exit 1
            fi
            rm -f /tmp/seen_call_shape_err.txt
        fi

        # NOTE: Phantom declare removal disabled — it was too aggressive and removed
        # declares for cross-module functions (emitIncludeStrImpl, etc.) that ARE called
        # from the module via ThinLTO. The awk dedup + fix_ir.py handle the critical cases.

        # IR Validation: run llvm-as structural check on the fixed .ll file
        if ! llvm-as "\$arg" -o /dev/null 2>/tmp/seen_verify_err.txt; then
            echo "IR VERIFY WARNING: \$arg (retrying fixups)" >&2
            head -2 /tmp/seen_verify_err.txt >&2
            if python3 "$SCRIPT_DIR/fix_ir.py" "\$arg" 2>&1 && \
               repair_stale_builder_ir "\$arg" && \
               llvm-as "\$arg" -o /dev/null 2>/tmp/seen_verify_err.txt; then
                echo "IR VERIFY RECOVERED: \$arg" >&2
            else
                echo "IR VERIFY ERROR: \$arg still invalid after retry" >&2
                head -2 /tmp/seen_verify_err.txt >&2
                rm -f /tmp/seen_verify_err.txt
                exit 1
            fi
            rm -f /tmp/seen_verify_err.txt
        fi
        rm -f /tmp/seen_verify_err.txt

        # NOTE: seen_ir_lint disabled — it naively counts commas to determine
        # argument count, causing false positives on LLVM inline struct literals
        # like %SeenString { i64 7, ptr @.str }. verify_ir_call_shapes.py handles
        # direct call signatures without those comma-splitting false positives.
    fi
done
if [ "\${SEEN_LOW_MEMORY:-0}" = "1" ]; then
    acquire_seen_low_memory_opt_lock
fi
exec "$REAL_OPT" "\$@"
WRAPPER_EOF
    chmod +x "$OPT_WRAPPER_DIR/opt"
fi  # end platform-specific opt wrapper

if [ "$REBUILD_TIER" = "full" ]; then
    if tier_builder_supports_jobs "$FROZEN_ABS"; then
        STAGE2_COMPILE_FLAGS="$STAGE2_COMPILE_FLAGS --jobs $SEEN_JOBS --opt-jobs $SEEN_OPT_JOBS"
    elif tier_builder_supports_no_fork "$FROZEN_ABS"; then
        STAGE2_COMPILE_FLAGS="$STAGE2_COMPILE_FLAGS --no-fork"
    elif [ -n "$FORK_SERIALIZER_SO" ]; then
        echo -e "${YELLOW}Frozen builder has no worker controls; required fork serialization is active.${NC}"
    else
        echo -e "${RED}ERROR: frozen builder has no bounded worker controls or serializer.${NC}" >&2
        exit 1
    fi
fi

if [ "$LOW_MEMORY_MODE" = "1" ] && [ "$HOST_OS" != "Darwin" ] && [ "${SEEN_SKIP_LOW_MEMORY_SHORTCUT:-0}" != "1" ]; then
    echo ""
    echo "Low-memory shortcut: trying existing stage builders before frozen bootstrap..."
    if recover_with_existing_stage_builders; then
        echo -e "${GREEN}Low-memory rebuild succeeded via existing stage builder.${NC}"
    elif [ "${SEEN_STAGE_BUILDER_ONLY:-0}" = "1" ]; then
        echo -e "${RED}ERROR: SEEN_STAGE_BUILDER_ONLY=1 and the selected stage builder failed.${NC}"
        exit 1
    elif recover_with_preserved_production_compiler; then
        echo -e "${GREEN}Low-memory rebuild succeeded via preserved production compiler.${NC}"
    else
        echo -e "${YELLOW}Low-memory shortcut did not complete; continuing with capped frozen bootstrap.${NC}"
    fi
elif [ "$LOW_MEMORY_MODE" = "1" ] && [ "$HOST_OS" != "Darwin" ]; then
    echo ""
    echo -e "${YELLOW}Low-memory shortcut skipped; continuing with capped frozen bootstrap.${NC}"
fi

if [ -z "${VERIFIED:-}" ]; then
# Step 1: Build stage2 with frozen compiler (--fast)
# NOTE: PATH override ensures our dedup opt wrapper runs instead of system opt.
echo ""
echo "Step 1: Building stage2 with frozen compiler (--fast)..."
echo -e "${DIM}The frozen compiler generates IR for all 50+ modules.${NC}"
echo -e "${DIM}Module 5 (llvm_ir_gen.seen, 14K lines) typically takes 1-2 minutes.${NC}"
echo ""

# Clean stale .ll/.o from previous runs so counts are accurate
rm -f /tmp/seen_module_*.ll /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll
rm -f /tmp/seen_module_*.opt.status /tmp/seen_module_*.opt.log

if [ "$HOST_OS" = "Darwin" ]; then
    # macOS: PATH already set via export above; use --no-cache
    # The frozen compiler may fail at its internal link step (e.g., internal globals
    # eliminated by opt) but still produce a full .opt.ll set we can relink in step 1b.
    if run_with_progress "S1→S2" /tmp/safe_rebuild_stage2.log \
        bash -c 'cd "$1" || exit 1; shift; exec "$@"' bash "$BOOTSTRAP_SOURCE_ROOT" \
        env -u SEEN_FORK_SERIALIZER_ROOT_PID \
            SEEN_PACKAGE_CLIENT="$SOURCE_PACKAGE_CLIENT" \
            SEEN_FROZEN_IR_COMPAT=1 \
            LD_PRELOAD="$FORK_SERIALIZER_SO" \
            SEEN_FORK_SERIALIZER_TARGET="$FROZEN_ABS" \
            SEEN_COMPILER_SOURCE_ROOT="$BOOTSTRAP_SOURCE_ROOT" \
            "$FROZEN_ABS" compile "$COMPILER_SOURCE" "$STAGE2" $STAGE2_COMPILE_FLAGS; then
        echo -e "${GREEN}Stage2 build succeeded.${NC}"
    else
        EXPECTED_STAGE2_MODULES=$(extract_expected_module_count /tmp/safe_rebuild_stage2.log)
        OPT_LL_COUNT=$(count_module_opt_lls /tmp)
        if [ "$EXPECTED_STAGE2_MODULES" -gt 0 ] && [ "$OPT_LL_COUNT" -eq "$EXPECTED_STAGE2_MODULES" ]; then
            echo -e "${YELLOW}Stage2 internal link failed, but the full $OPT_LL_COUNT/$EXPECTED_STAGE2_MODULES .opt.ll set is available for relink.${NC}"
        else
            echo -e "${RED}ERROR: Stage2 build failed!${NC}"
            if [ "$EXPECTED_STAGE2_MODULES" -gt 0 ]; then
                echo "Expected $EXPECTED_STAGE2_MODULES optimized modules, found $OPT_LL_COUNT."
            fi
            echo "Check /tmp/safe_rebuild_stage2.log for details."
            tail_log_if_exists /tmp/safe_rebuild_stage2.log 30
            exit 1
        fi
    fi
else
    # Linux S1→S2: Inline process management so we can run a snapshot watcher.
    # The frozen compiler deletes ALL /tmp/seen_module_* on exit (even on failure),
    # so we snapshot .ll files while it's running.
    SNAPSHOT_DIR="/tmp/seen_ll_snapshot_$$"
    rm -rf "$SNAPSHOT_DIR"

    FROZEN_COMPILE_ENV=(env -u SEEN_FORK_SERIALIZER_ROOT_PID \
        "SEEN_PACKAGE_CLIENT=$SOURCE_PACKAGE_CLIENT" \
        "SEEN_FROZEN_IR_COMPAT=1" \
        "PATH=$OPT_WRAPPER_DIR:$PATH" \
        "SEEN_COMPILER_SOURCE_ROOT=$BOOTSTRAP_SOURCE_ROOT")
    if [ -n "$FORK_SERIALIZER_SO" ]; then
        FROZEN_COMPILE_ENV+=("LD_PRELOAD=$FORK_SERIALIZER_SO")
        FROZEN_COMPILE_ENV+=("SEEN_FORK_SERIALIZER_TARGET=$FROZEN_ABS")
        FROZEN_COMPILE_ENV+=("SEEN_FORK_SERIALIZER_DESCENDANT_SCRIPT=/tmp/seen_parallel_opt.sh")
    fi

    # Start compiler in background
    SEEN_MEMORY_GUARD_KILL_ONLY=1 run_guarded_command_to_log "S1->S2" 0 "$MAIN_COMPILER_VMEM_KB" /tmp/safe_rebuild_stage2.log \
        bash -c 'cd "$1" || exit 1; shift; exec "$@"' bash "$BOOTSTRAP_SOURCE_ROOT" \
        "${FROZEN_COMPILE_ENV[@]}" "$FROZEN_ABS" compile "$COMPILER_SOURCE" "$STAGE2" \
            $STAGE2_COMPILE_FLAGS &
    COMPILE_PID=$!

    # Start progress monitor and snapshot watcher
    monitor_compilation "$COMPILE_PID" "S1→S2" &
    MONITOR_PID=$!
    FAILURE_WATCHER_PID=$(start_log_failure_watcher "S1→S2" /tmp/safe_rebuild_stage2.log "$COMPILE_PID")
    start_ll_snapshot_watcher "$COMPILE_PID" "$SNAPSHOT_DIR" &
    WATCHER_PID=$!

    # Wait for compiler
    COMPILE_EXIT=0
    wait "$COMPILE_PID" || COMPILE_EXIT=$?

    # Stop monitor and watcher
    kill "$MONITOR_PID" 2>/dev/null || true
    wait "$MONITOR_PID" 2>/dev/null || true
    if [ -n "$FAILURE_WATCHER_PID" ]; then
        kill "$FAILURE_WATCHER_PID" 2>/dev/null || true
        wait "$FAILURE_WATCHER_PID" 2>/dev/null || true
    fi
    finish_ll_snapshot_watcher "$WATCHER_PID"

    EXPECTED_STAGE2_MODULES=$(extract_expected_module_count /tmp/safe_rebuild_stage2.log)
    if [ "$EXPECTED_STAGE2_MODULES" -le 0 ]; then
        echo -e "${RED}ERROR: Could not determine expected module count for Stage2.${NC}"
        echo "Check /tmp/safe_rebuild_stage2.log for details."
        tail_log_if_exists /tmp/safe_rebuild_stage2.log 30
        preserve_stage2_failure_artifacts "$SNAPSHOT_DIR"
        rm -rf "$SNAPSHOT_DIR"
        exit 1
    fi

    if [ "$COMPILE_EXIT" -eq 0 ]; then
        STAGE2_OBJ_COUNT=$(count_module_objects /tmp)
        if [ "$STAGE2_OBJ_COUNT" -ne "$EXPECTED_STAGE2_MODULES" ]; then
            echo -e "${RED}ERROR: Stage2 reported success but produced only $STAGE2_OBJ_COUNT/$EXPECTED_STAGE2_MODULES module objects.${NC}"
            echo "Check /tmp/safe_rebuild_stage2.log for details."
            tail_log_if_exists /tmp/safe_rebuild_stage2.log 30
            preserve_stage2_failure_artifacts "$SNAPSHOT_DIR"
            rm -rf "$SNAPSHOT_DIR"
            exit 1
        fi
        echo -e "${GREEN}Stage2 build succeeded.${NC}"
        rm -rf "$SNAPSHOT_DIR"
    else
        if stage2_failure_looks_oom "$COMPILE_EXIT" /tmp/safe_rebuild_stage2.log; then
            echo -e "${RED}ERROR: Stage2 hit an OOM/resource failure; aborting without partial-IR recovery or retry.${NC}" >&2
            kill_frozen_orphans
            preserve_stage2_failure_artifacts "$SNAPSHOT_DIR"
            exit "$COMPILE_EXIT"
        fi

        # Kill orphaned fork children before recovery (SIGKILL to avoid cleanup handlers)
        echo -e "${YELLOW}Stage2 compilation failed (exit=$COMPILE_EXIT), killing orphans...${NC}"
        kill_frozen_orphans
        preserve_stage2_failure_artifacts "$SNAPSHOT_DIR"
        summarize_stage2_failure_log /tmp/safe_rebuild_stage2.log
        if [ "${SEEN_STOP_AFTER_FROZEN_STAGE2_FAILURE:-0}" = "1" ] ||
           [ "${SEEN_STAGE2_FAIL_FAST:-0}" = "1" ]; then
            echo -e "${RED}Stopping after frozen Stage2 failure as requested.${NC}"
            echo "Set SEEN_STAGE2_FAIL_FAST=0 to allow direct IR recovery."
            rm -rf "$SNAPSHOT_DIR"
            exit "$COMPILE_EXIT"
        fi
        if [ "$IR_RECOVERY_DISABLED" = "1" ]; then
            echo -e "${RED}ERROR: Stage2 failed and SEEN_DISABLE_IR_RECOVERY=1 forbids direct IR recovery.${NC}"
            rm -rf "$SNAPSHOT_DIR"
            exit "$COMPILE_EXIT"
        fi

        # Check how many .ll files we have: first from snapshot, fallback to live /tmp
        SNAP_COUNT=$(count_plain_module_lls "$SNAPSHOT_DIR")
        LIVE_COUNT=$(count_plain_module_lls /tmp)

        LL_RECOVERY_SOURCE_DIR=""
        if [ "$SNAP_COUNT" -gt "$LIVE_COUNT" ]; then
            LL_COUNT=$SNAP_COUNT
            LL_SOURCE="snapshot"
            echo -e "${YELLOW}Snapshot has $SNAP_COUNT .ll files (live: $LIVE_COUNT). Restoring from snapshot...${NC}"
            # Clean /tmp of any partial files, then restore from snapshot
            rm -f /tmp/seen_module_*.ll /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll
            rm -f /tmp/seen_module_*.opt.status /tmp/seen_module_*.opt.log
            cp "$SNAPSHOT_DIR"/seen_module_*.ll /tmp/ 2>/dev/null || true
            LL_RECOVERY_SOURCE_DIR="$SNAPSHOT_DIR"
        else
            LL_COUNT=$LIVE_COUNT
            LL_SOURCE="live"
            echo -e "${YELLOW}Using $LIVE_COUNT live .ll files from /tmp.${NC}"
            LL_RECOVERY_SOURCE_DIR="/tmp"
        fi

        SKIP_PRESERVED_RECOVERY=0
        if [ "$LL_COUNT" -eq "$EXPECTED_STAGE2_MODULES" ]; then
            SKIP_PRESERVED_RECOVERY=1
            echo -e "${YELLOW}Captured a full $LL_COUNT/$EXPECTED_STAGE2_MODULES .ll set; skipping preserved-compiler rebuild and recovering directly from IR.${NC}"
            FULL_LL_RECOVERY_DIR="/tmp/seen_full_ll_recovery_$$"
            rm -rf "$FULL_LL_RECOVERY_DIR"
            mkdir -p "$FULL_LL_RECOVERY_DIR"
            cp "$LL_RECOVERY_SOURCE_DIR"/seen_module_*.ll "$FULL_LL_RECOVERY_DIR/" 2>/dev/null || true
            FULL_LL_COUNT=$(count_plain_module_lls "$FULL_LL_RECOVERY_DIR")
            if [ "$FULL_LL_COUNT" -ne "$EXPECTED_STAGE2_MODULES" ]; then
                echo -e "${RED}ERROR: failed to preserve complete .ll recovery set ($FULL_LL_COUNT/$EXPECTED_STAGE2_MODULES).${NC}"
                rm -rf "$SNAPSHOT_DIR" "$FULL_LL_RECOVERY_DIR"
                exit 1
            fi
            LL_RECOVERY_SOURCE_DIR="$FULL_LL_RECOVERY_DIR"
        fi

        if [ "$SKIP_PRESERVED_RECOVERY" -eq 0 ] && recover_with_preserved_production_compiler; then
            echo -e "${YELLOW}Frozen Stage2 bootstrap failed; skipping slow .ll replay recovery and using the preserved-compiler recovery build.${NC}"
        elif [ "$LL_COUNT" -eq "$EXPECTED_STAGE2_MODULES" ]; then
            echo -e "${YELLOW}Recovering with the full $LL_COUNT/$EXPECTED_STAGE2_MODULES .ll set ($LL_SOURCE)...${NC}"

            # Clean stale .o and .opt.ll from the compiler's failed internal opt/link —
            # we must regenerate them from the raw .ll files via our own opt wrapper.
            rm -f /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll
            rm -f /tmp/seen_module_*.opt.status /tmp/seen_module_*.opt.log

            # Run recovery in subprocess (immune to set -e).
            # Recovery works in a private temp dir to avoid interference from
            # concurrent compilations. It outputs RECOVERY_DIR=<path> on success.
            RECOVERY_EXIT=0
            RECOVERY_LOG="/tmp/seen_stage2_recovery_$$.log"
            set +e
            run_guarded_command "Stage2 frozen-IR compatibility" "$RECOVERY_TIMEOUT_SECS" "$OPT_VMEM_KB" \
                env SEEN_FROZEN_IR_COMPAT=1 \
                bash "$SCRIPT_DIR/recovery_opt.sh" "$OPT_WRAPPER_DIR" "$SCRIPT_DIR" "$LL_RECOVERY_SOURCE_DIR" 2>&1 | tee "$RECOVERY_LOG"
            RECOVERY_EXIT=${PIPESTATUS[0]}
            set -e
            RECOVERY_OUTPUT=$(cat "$RECOVERY_LOG" 2>/dev/null || true)
            rm -f "$RECOVERY_LOG"

            if [ "$RECOVERY_EXIT" -ne 0 ]; then
                echo -e "${RED}ERROR: Recovery failed.${NC}"
                rm -rf "$SNAPSHOT_DIR" "$FULL_LL_RECOVERY_DIR"
                exit 1
            fi

            RECOVERY_DIR=$(echo "$RECOVERY_OUTPUT" | grep '^RECOVERY_DIR=' | tail -1 | cut -d= -f2)
            if [ -z "$RECOVERY_DIR" ] || [ ! -d "$RECOVERY_DIR" ]; then
                echo -e "${RED}ERROR: Recovery failed — no output directory.${NC}"
                rm -rf "$SNAPSHOT_DIR" "$FULL_LL_RECOVERY_DIR"
                exit 1
            fi

            OBJ_COUNT=$(count_module_objects "$RECOVERY_DIR")
            if [ "$OBJ_COUNT" -ne "$EXPECTED_STAGE2_MODULES" ]; then
                MISSING_MODULES=$(list_modules_missing_objects "$RECOVERY_DIR")
                echo -e "${RED}ERROR: Recovery failed — only $OBJ_COUNT/$EXPECTED_STAGE2_MODULES .o files produced.${NC}"
                if [ -n "$MISSING_MODULES" ]; then
                    echo "Missing objects:$MISSING_MODULES"
                fi
                rm -rf "$SNAPSHOT_DIR" "$FULL_LL_RECOVERY_DIR" "$RECOVERY_DIR"
                exit 1
            fi
            echo "  Recovery: $OBJ_COUNT/$EXPECTED_STAGE2_MODULES .o files ready."

            # Check for empty modules that might cause link failures.
            # Skip modules that are legitimately empty (only declares/types, no string
            # constants — these are re-export shims with no real code).
            EMPTY_MODULES=$(find_problem_empty_modules "$RECOVERY_DIR")
            EMPTY_COUNT=$(echo "$EMPTY_MODULES" | wc -w)
            if [ "$EMPTY_COUNT" -gt 0 ]; then
                echo -e "${YELLOW}Empty modules ($EMPTY_COUNT with 0 function definitions):${EMPTY_MODULES}${NC}"

                # Never re-run a compiler after a partial or resource-stopped
                # attempt. Continue only with deterministic merge/recovery from
                # already captured artifacts and bounded trusted builders.
                echo -e "${YELLOW}Frozen compiler retries are disabled; using deterministic captured-artifact recovery only.${NC}"

                # --- Pass 2: Two-pass .ll merge for satellite modules ---
                if [ "$EMPTY_COUNT" -gt 0 ]; then
                # The frozen compiler generates module 5 (llvm_ir_gen.seen) correctly
                # but outputs 0 defines for satellite codegen modules. The production
                # compiler generates satellite modules correctly but hangs on module 5.
                # Merge: pick the .ll with more defines from each compiler.
                PROD_COMPILER="$REPO_ROOT/compiler_seen/target/seen"
                if [ -x "$PROD_COMPILER" ]; then
                    if ! compiler_serializer_applicable "$PROD_COMPILER"; then
                        echo -e "${YELLOW}Production compiler failed serializer applicability checks; Pass 2 rejected.${NC}"
                        PROD_COMPILER=""
                    fi
                fi
                if [ -x "$PROD_COMPILER" ]; then
                    if tier_builder_supports_jobs "$PROD_COMPILER"; then
                        PASS2_COMPILE_FLAGS="$PASS2_COMPILE_FLAGS --jobs $SEEN_JOBS --opt-jobs $SEEN_OPT_JOBS"
                    elif tier_builder_supports_no_fork "$PROD_COMPILER"; then
                        PASS2_COMPILE_FLAGS="$PASS2_COMPILE_FLAGS --no-fork"
                    else
                        echo -e "${YELLOW}Production compiler advertises no worker controls; direct-child serialization and the hard cgroup remain required for Pass 2.${NC}"
                    fi
                fi
                if [ -x "$PROD_COMPILER" ]; then
                    echo -e "${YELLOW}Running Pass 2 (production compiler) to fill empty modules...${NC}"

                    # Clean /tmp for Pass 2
                    rm -f /tmp/seen_module_*.ll /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll
                    rm -f /tmp/seen_module_*.opt.status /tmp/seen_module_*.opt.log

                    # Run production compiler with timeout + snapshot watcher
                    PASS2_SNAPSHOT="/tmp/seen_pass2_snapshot_$$"
                    rm -rf "$PASS2_SNAPSHOT"

                    SEEN_MEMORY_GUARD_KILL_ONLY=1 run_guarded_command_to_log "Pass2" 600 "$MAIN_COMPILER_VMEM_KB" /tmp/pass2.log \
                        bash -c 'cd "$1" || exit 1; shift; exec "$@"' bash "$BOOTSTRAP_SOURCE_ROOT" \
                        env -u SEEN_FORK_SERIALIZER_ROOT_PID -u SEEN_PACKAGE_CLIENT \
                        LD_PRELOAD="$FORK_SERIALIZER_SO" \
                        SEEN_FORK_SERIALIZER_TARGET="$PROD_COMPILER" \
                        SEEN_COMPILER_SOURCE_ROOT="$BOOTSTRAP_SOURCE_ROOT" \
                        "$PROD_COMPILER" compile "$COMPILER_SOURCE" /dev/null \
                            $PASS2_COMPILE_FLAGS &
                    PASS2_PID=$!

                    start_ll_snapshot_watcher "$PASS2_PID" "$PASS2_SNAPSHOT" &
                    PASS2_WATCHER=$!
                    monitor_compilation "$PASS2_PID" "Pass2" &
                    PASS2_MONITOR=$!

                    wait $PASS2_PID 2>/dev/null || true
                    finish_ll_snapshot_watcher "$PASS2_WATCHER"
                    kill $PASS2_MONITOR 2>/dev/null; wait $PASS2_MONITOR 2>/dev/null || true
                    # The per-command guard owns Pass 2's process group. Do not
                    # scan or signal host processes after its PID has exited.
                    sleep 2

                    PASS2_COUNT=$(count_plain_module_lls "$PASS2_SNAPSHOT")
                    echo ""
                    echo "  Pass 2: captured $PASS2_COUNT .ll files"

                    # Merge: for each module, pick the .ll with more defines
                    MERGED=0
                    for pass1_ll in "$RECOVERY_DIR"/seen_module_*.ll; do
                        [ -f "$pass1_ll" ] || continue
                        [[ "$pass1_ll" == *.opt.ll ]] && continue
                        [[ "$pass1_ll" == *.polly.ll ]] && continue
                        bn=$(basename "$pass1_ll")
                        pass2_ll="$PASS2_SNAPSHOT/$bn"
                        [ -f "$pass2_ll" ] || continue

                        p1_defines=$(grep -c '^define' "$pass1_ll" 2>/dev/null | tail -1)
                        p1_defines=${p1_defines:-0}
                        p2_defines=$(grep -c '^define' "$pass2_ll" 2>/dev/null | tail -1)
                        p2_defines=${p2_defines:-0}

                        if [ "$p2_defines" -gt "$p1_defines" ] 2>/dev/null; then
                            cp "$pass2_ll" "$pass1_ll"
                            modname=$(basename "$pass1_ll" .ll)
                            rm -f "$RECOVERY_DIR/${modname}.opt.ll" "$RECOVERY_DIR/${modname}.o"
                            echo "    Merged $bn: $p1_defines -> $p2_defines defines (from production)"
                            MERGED=$((MERGED+1))
                        fi
                    done
                    # Also add Pass 2 .ll files not present in RECOVERY_DIR
                    for pass2_ll in "$PASS2_SNAPSHOT"/seen_module_*.ll; do
                        [ -f "$pass2_ll" ] || continue
                        [[ "$pass2_ll" == *.opt.ll ]] && continue
                        [[ "$pass2_ll" == *.polly.ll ]] && continue
                        bn=$(basename "$pass2_ll")
                        if [ ! -f "$RECOVERY_DIR/$bn" ]; then
                            cp "$pass2_ll" "$RECOVERY_DIR/$bn"
                            echo "    Added $bn from Pass 2 (not in Pass 1)"
                            MERGED=$((MERGED+1))
                        fi
                    done
                    rm -rf "$PASS2_SNAPSHOT"
                    echo "  Merged $MERGED modules from Pass 2"

                    if [ "$MERGED" -gt 0 ]; then
                        # Re-run opt + thinlto-bc on merged modules (those missing .o files)
                        REAL_OPT_BIN=$(command -v opt)
                        for llfile in "$RECOVERY_DIR"/seen_module_*.ll; do
                            [ -f "$llfile" ] || continue
                            [[ "$llfile" == *.opt.ll ]] && continue
                            [[ "$llfile" == *.polly.ll ]] && continue
                            modname=$(basename "$llfile" .ll)
                            objfile="$RECOVERY_DIR/${modname}.o"
                            [ -f "$objfile" ] && continue

                            optfile="$RECOVERY_DIR/${modname}.opt.ll"
                            echo "    Re-optimizing $modname..."
                            if ! run_guarded_command "Pass2 ${modname} opt" 300 "$OPT_VMEM_KB" \
                                "$REAL_OPT_BIN" \
                                -passes='function(sroa,instcombine<no-verify-fixpoint>,simplifycfg),default<O1>' \
                                -inline-threshold=250 -S "$llfile" -o "$optfile" 2>/dev/null; then
                                cp "$llfile" "$optfile" 2>/dev/null || true
                            fi
                            run_guarded_command "Pass2 ${modname} thinlto" 300 "$OPT_VMEM_KB" \
                                "$REAL_OPT_BIN" --thinlto-bc "$optfile" -o "$objfile" 2>/dev/null || true
                        done

                        # Recount .o files after merge
                        OBJ_COUNT=$(count_module_objects "$RECOVERY_DIR")
                        echo "  Post-merge: $OBJ_COUNT/$EXPECTED_STAGE2_MODULES .o files ready."
                    fi
                else
                    echo -e "${YELLOW}No production compiler available for Pass 2 merge.${NC}"
                    echo -e "${YELLOW}Fix: revert module list changes, rebuild, update stage1_frozen, re-apply.${NC}"
                fi
                fi
            fi

            EMPTY_MODULES=$(find_problem_empty_modules "$RECOVERY_DIR")
            EMPTY_COUNT=$(echo "$EMPTY_MODULES" | wc -w)
            if [ "$EMPTY_COUNT" -gt 0 ]; then
                echo -e "${RED}ERROR: Recovery left $EMPTY_COUNT module(s) with missing function bodies:${EMPTY_MODULES}${NC}"
                rm -rf "$SNAPSHOT_DIR" "$FULL_LL_RECOVERY_DIR" "$RECOVERY_DIR"
                exit 1
            fi

            OBJ_COUNT=$(count_module_objects "$RECOVERY_DIR")
            if [ "$OBJ_COUNT" -ne "$EXPECTED_STAGE2_MODULES" ]; then
                MISSING_MODULES=$(list_modules_missing_objects "$RECOVERY_DIR")
                echo -e "${RED}ERROR: Recovery object set is incomplete ($OBJ_COUNT/$EXPECTED_STAGE2_MODULES).${NC}"
                if [ -n "$MISSING_MODULES" ]; then
                    echo "Missing objects:$MISSING_MODULES"
                fi
                rm -rf "$SNAPSHOT_DIR" "$FULL_LL_RECOVERY_DIR" "$RECOVERY_DIR"
                exit 1
            fi

            # Pre-compile runtime
            RT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)/seen_runtime"
            if [ ! -f "$RT_DIR/seen_runtime.o" ] || [ "$RT_DIR/seen_runtime.c" -nt "$RT_DIR/seen_runtime.o" ]; then
                echo "  Pre-compiling runtime..."
                run_guarded_command "runtime seen_runtime.c" 300 "$OPT_VMEM_KB" \
                    clang -O3 -flto=thin "$RELEASE_CLANG_MARCH_FLAG" -ffunction-sections -fdata-sections -pthread \
                    -c -I "$RT_DIR" "$RT_DIR/seen_runtime.c" -o "$RT_DIR/seen_runtime.o" 2>/dev/null || true
            fi
            if [ -f "$RT_DIR/seen_region.c" ]; then
                if [ ! -f "$RT_DIR/seen_region.o" ] || [ "$RT_DIR/seen_region.c" -nt "$RT_DIR/seen_region.o" ]; then
                    run_guarded_command "runtime seen_region.c" 300 "$OPT_VMEM_KB" \
                        clang -O3 -flto=thin "$RELEASE_CLANG_MARCH_FLAG" -ffunction-sections -fdata-sections \
                        -c -I "$RT_DIR" "$RT_DIR/seen_region.c" -o "$RT_DIR/seen_region.o" 2>/dev/null || true
                fi
            fi
            if [ -f "$RT_DIR/seen_gpu.c" ]; then
                if [ ! -f "$RT_DIR/seen_gpu.o" ] || [ "$RT_DIR/seen_gpu.c" -nt "$RT_DIR/seen_gpu.o" ]; then
                    run_guarded_command "runtime seen_gpu.c" 300 "$OPT_VMEM_KB" \
                        clang -O3 -flto=thin "$RELEASE_CLANG_MARCH_FLAG" -ffunction-sections -fdata-sections \
                        -c -I "$RT_DIR" "$RT_DIR/seen_gpu.c" -o "$RT_DIR/seen_gpu.o" 2>/dev/null || true
                fi
            fi

            # Link from recovery directory (not /tmp, which may be contaminated
            # by concurrent compilations)
            echo "  Linking $OBJ_COUNT modules..."
            LINK_OBJS=""
            for obj in "$RECOVERY_DIR"/seen_module_*.o; do
                LINK_OBJS="$LINK_OBJS $obj"
            done
            RT_OBJS="$RT_DIR/seen_runtime.o"
            [ -f "$RT_DIR/seen_region.o" ] && RT_OBJS="$RT_OBJS $RT_DIR/seen_region.o"
            [ -f "$RT_DIR/seen_gpu.o" ] && RT_OBJS="$RT_OBJS $RT_DIR/seen_gpu.o"

            LINK_LIBS="-lm -lpthread"
            [ -f "$RT_DIR/seen_gpu.o" ] && pkg-config --exists vulkan 2>/dev/null && LINK_LIBS="$LINK_LIBS -lvulkan"

            if run_guarded_command "Stage2 recovery link" 0 "$OPT_VMEM_KB" clang -O1 -fuse-ld=lld \
                -Wl,--allow-multiple-definition \
                "$RELEASE_CLANG_MARCH_FLAG" -Wl,--gc-sections -Wno-unused-command-line-argument \
                $LINK_OBJS $RT_OBJS -o "$STAGE2" $LINK_LIBS 2>/tmp/safe_rebuild_link.log; then
                echo -e "${GREEN}Stage2 recovery link succeeded ($(wc -c < "$STAGE2" | tr -d ' ') bytes).${NC}"
            else
                echo -e "${RED}ERROR: Stage2 recovery link failed.${NC}"
                grep -E 'undefined|error' /tmp/safe_rebuild_link.log | head -10
                rm -rf "$SNAPSHOT_DIR" "$FULL_LL_RECOVERY_DIR" "$RECOVERY_DIR"
                exit 1
            fi
            rm -rf "$FULL_LL_RECOVERY_DIR" "$RECOVERY_DIR"
        else
            echo -e "${RED}ERROR: Stage2 build failed after generating only $LL_COUNT/$EXPECTED_STAGE2_MODULES .ll files from $LL_SOURCE.${NC}"
            echo "Check /tmp/safe_rebuild_stage2.log for details."
            tail_log_if_exists /tmp/safe_rebuild_stage2.log 30
            exit 1
        fi
        rm -rf "$SNAPSHOT_DIR" "$FULL_LL_RECOVERY_DIR"
    fi
fi

# macOS relink: The frozen bootstrap produces ThinLTO bitcode .o files which
# don't link correctly without -flto=thin. Relink from .opt.ll using llc + clang.
if [ "$HOST_OS" = "Darwin" ]; then
    echo ""
    echo "Step 1b: macOS relink from .opt.ll files (llc + clang -O2)..."

    # Post-opt fixup: LLVM opt may eliminate internal globals that are cross-module referenced.
    # Scan all .opt.ll files: for each 'external global @X' reference, ensure @X is defined
    # (non-external) in at least one module. If eliminated, re-inject from the original .ll.
    echo "    Fixing cross-module globals post-opt..."
    "$PYTHON3_PATH" - <<'POSTOPT_FIX'
import re, glob, os

opt_files = sorted(glob.glob('/tmp/seen_module_*.opt.ll'))
ll_files = sorted(glob.glob('/tmp/seen_module_*.ll'))
ll_files = [f for f in ll_files if not f.endswith('.opt.ll') and not f.endswith('.polly.ll')]

# Collect: which opt files define globals, which declare them external
defined = {}   # gname -> (file, line)
external = {}  # gname -> set of files needing it

for f in opt_files:
    with open(f) as fh:
        for line in fh:
            gm = re.match(r'(@\w+)\s*=\s*(external\s+)?(?:local_unnamed_addr\s+)?(?:unnamed_addr\s+)?(?:internal\s+)?global\s+(\S+)', line)
            if gm:
                gname = gm.group(1)
                is_ext = gm.group(2) is not None
                if is_ext:
                    external.setdefault(gname, set()).add(f)
                else:
                    defined[gname] = (f, gm.group(3))

# Find globals referenced but not defined in any opt file
missing = set()
for gname in external:
    if gname not in defined:
        missing.add(gname)

if missing:
    # Try to find definitions in original .ll files
    orig_defs = {}
    for f in ll_files:
        with open(f) as fh:
            for line in fh:
                gm = re.match(r'(@\w+)\s*=\s*(?:internal\s+)?global\s+(\S+)\s+(.*)', line)
                if gm and gm.group(1) in missing:
                    orig_defs[gm.group(1)] = (gm.group(2), gm.group(3).strip(), f)

    # Inject missing globals into the first module that references them externally
    for gname in missing:
        if gname in orig_defs:
            gtype, gval, src = orig_defs[gname]
            # Add to first referencing opt file
            target = sorted(external[gname])[0]
            with open(target) as fh:
                content = fh.read()
            # Replace external declaration with actual definition
            content = re.sub(
                rf'^{re.escape(gname)}\s*=\s*external\s+(?:local_unnamed_addr\s+)?(?:unnamed_addr\s+)?global\s+\S+\s*$',
                f'{gname} = global {gtype} {gval}',
                content, count=1, flags=re.MULTILINE
            )
            with open(target, 'w') as fh:
                fh.write(content)
            print(f'    Injected {gname} into {os.path.basename(target)}')

if not missing:
    print('    Post-opt fixup: 0 missing globals')
else:
    print(f'    Post-opt fixup: {len(missing)} missing globals, {len(missing & set(orig_defs.keys()))} fixed')
POSTOPT_FIX

    EXPECTED_STAGE2_MODULES=$(extract_expected_module_count /tmp/safe_rebuild_stage2.log)
    OPT_LL_COUNT=$(count_module_opt_lls /tmp)
    if [ "$EXPECTED_STAGE2_MODULES" -le 0 ]; then
        echo -e "${RED}ERROR: Could not determine expected module count for macOS relink.${NC}"
        exit 1
    fi
    if [ "$OPT_LL_COUNT" -ne "$EXPECTED_STAGE2_MODULES" ]; then
        echo -e "${RED}ERROR: Refusing macOS relink with only $OPT_LL_COUNT/$EXPECTED_STAGE2_MODULES optimized modules.${NC}"
        exit 1
    fi
    if [ "$OPT_LL_COUNT" -gt 0 ]; then
        echo "    Found $OPT_LL_COUNT .opt.ll modules, relinking with llc..."
        RELINK_FAILED=0
        RELINK_OBJS=""
        for optll in /tmp/seen_module_*.opt.ll; do
            modname=$(basename "$optll" .opt.ll)
            objfile="/tmp/${modname}.relink.o"
            if ! run_guarded_command "macOS stage2 ${modname} llc" 300 "$OPT_VMEM_KB" \
                llc -mtriple=arm64-apple-macosx -filetype=obj -O2 "$optll" -o "$objfile" 2>/tmp/relink_llc.log; then
                echo -e "${RED}    llc failed for $modname${NC}"
                cat /tmp/relink_llc.log
                RELINK_FAILED=1
                break
            fi
            RELINK_OBJS="$RELINK_OBJS $objfile"
        done
        if [ "$RELINK_FAILED" = "0" ]; then
            NATIVE_RT="/tmp/seen_runtime_native.o"
            run_guarded_command "macOS stage2 runtime" 300 "$OPT_VMEM_KB" \
                clang -O2 -c -I seen_runtime seen_runtime/seen_runtime.c -o "$NATIVE_RT" 2>/dev/null || true
            NATIVE_REGION="/tmp/seen_region_native.o"
            [ -f seen_runtime/seen_region.c ] && run_guarded_command "macOS stage2 region" 300 "$OPT_VMEM_KB" \
                clang -O2 -c -I seen_runtime seen_runtime/seen_region.c -o "$NATIVE_REGION" 2>/dev/null || true
            RT_OBJS="$NATIVE_RT"
            [ -f "$NATIVE_REGION" ] && RT_OBJS="$RT_OBJS $NATIVE_REGION"
            if run_guarded_command "macOS stage2 relink" 0 "$OPT_VMEM_KB" \
                clang -O2 -arch arm64 $RELINK_OBJS $RT_OBJS -o "$STAGE2" -lm -lpthread 2>/tmp/relink_link.log; then
                echo -e "${GREEN}    macOS relink succeeded ($(wc -c < "$STAGE2" | tr -d ' ') bytes).${NC}"
            else
                echo -e "${RED}    macOS relink failed${NC}"
                cat /tmp/relink_link.log
                exit 1
            fi
            rm -f /tmp/seen_module_*.relink.o "$NATIVE_RT" "$NATIVE_REGION"
        else
            echo -e "${RED}ERROR: macOS relink failed${NC}"
            exit 1
        fi
    fi
fi
fi

if [ -z "${VERIFIED:-}" ]; then
echo ""
echo "Stage2 smoke: checking hello-world..."
if ! compiler_reports_checkout_version "$STAGE2"; then
    echo -e "${RED}ERROR: Fresh Stage2 does not report the exact checkout version.${NC}" >&2
    exit 1
fi
if ! smoke_test_compiler "$STAGE2" "Stage2" "stage2"; then
    echo -e "${RED}ERROR: Fresh Stage2 cannot compile a normal user program.${NC}"
    echo -e "${RED}Refusing to continue with an unusable bootstrap compiler.${NC}"
    exit 1
fi
if [ "${SEEN_STOP_AFTER_STAGE2_SMOKE:-0}" = "1" ]; then
    echo -e "${YELLOW}Stopping after Stage2 smoke as requested.${NC}"
    echo "Stage2 binary preserved at $STAGE2"
    exit 0
fi

STAGE3_COMPILE_FLAGS=(--fast --no-cache)
if tier_builder_supports_jobs "$STAGE2"; then
    STAGE3_COMPILE_FLAGS+=(--jobs "$SEEN_JOBS" --opt-jobs "$SEEN_OPT_JOBS")
elif tier_builder_supports_no_fork "$STAGE2"; then
    STAGE3_COMPILE_FLAGS+=(--no-fork)
else
    echo -e "${YELLOW}Fresh Stage2 advertises no worker controls; direct-child serialization and the hard cgroup remain mandatory.${NC}"
fi
if [ -n "${RELEASE_TARGET_CPU_FLAG:-}" ]; then
    STAGE3_COMPILE_FLAGS+=("$RELEASE_TARGET_CPU_FLAG")
fi

if [ "$HOST_OS" = "Darwin" ]; then
    # macOS: full S2→S3 bootstrap verification works
    rm -rf .seen_cache/ /tmp/seen_ir_cache/
    rm -f /tmp/seen_module_*.ll /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll
    rm -f /tmp/seen_module_*.opt.status /tmp/seen_module_*.opt.log

    echo ""
    echo "Step 2: Building stage3 with stage2 (--fast)..."
    if run_with_progress "S2→S3" /tmp/safe_rebuild_stage3.log \
        env -u SEEN_FORK_SERIALIZER_ROOT_PID \
        SEEN_PACKAGE_CLIENT="$SOURCE_PACKAGE_CLIENT" \
        SEEN_COMPILER_SOURCE_ROOT="$REPO_ROOT" \
        LD_PRELOAD="$FORK_SERIALIZER_SO" \
        SEEN_FORK_SERIALIZER_TARGET="$STAGE2" \
        "$STAGE2" compile "$COMPILER_SOURCE" "$STAGE3" "${STAGE3_COMPILE_FLAGS[@]}"; then
        echo -e "${GREEN}Stage3 build succeeded.${NC}"
    else
        echo -e "${RED}ERROR: Stage3 build failed!${NC}"
        echo "Check /tmp/safe_rebuild_stage3.log for details."
        tail_log_if_exists /tmp/safe_rebuild_stage3.log 30
        rm -f "$STAGE2"
        exit 1
    fi

    # Relink stage3
    echo ""
    echo "Step 2b: macOS relink stage3 from .opt.ll files..."
    EXPECTED_STAGE3_MODULES=$(extract_expected_module_count /tmp/safe_rebuild_stage3.log)
    OPT_LL_COUNT=$(count_module_opt_lls /tmp)
    if [ "$EXPECTED_STAGE3_MODULES" -le 0 ]; then
        echo -e "${RED}ERROR: Could not determine expected module count for macOS stage3 relink.${NC}"
        exit 1
    fi
    if [ "$OPT_LL_COUNT" -ne "$EXPECTED_STAGE3_MODULES" ]; then
        echo -e "${RED}ERROR: Refusing macOS stage3 relink with only $OPT_LL_COUNT/$EXPECTED_STAGE3_MODULES optimized modules.${NC}"
        exit 1
    fi
    if [ "$OPT_LL_COUNT" -gt 0 ]; then
        echo "    Found $OPT_LL_COUNT .opt.ll modules, relinking with llc..."
        RELINK_FAILED=0
        RELINK_OBJS=""
        for optll in /tmp/seen_module_*.opt.ll; do
            modname=$(basename "$optll" .opt.ll)
            objfile="/tmp/${modname}.relink.o"
            if ! run_guarded_command "macOS stage3 ${modname} llc" 300 "$OPT_VMEM_KB" \
                llc -mtriple=arm64-apple-macosx -filetype=obj -O2 "$optll" -o "$objfile" 2>/tmp/relink_llc.log; then
                echo -e "${RED}    llc failed for $modname${NC}"
                cat /tmp/relink_llc.log
                RELINK_FAILED=1
                break
            fi
            RELINK_OBJS="$RELINK_OBJS $objfile"
        done
        if [ "$RELINK_FAILED" = "0" ]; then
            NATIVE_RT="/tmp/seen_runtime_native.o"
            run_guarded_command "macOS stage3 runtime" 300 "$OPT_VMEM_KB" \
                clang -O2 -c -I seen_runtime seen_runtime/seen_runtime.c -o "$NATIVE_RT" 2>/dev/null || true
            NATIVE_REGION="/tmp/seen_region_native.o"
            [ -f seen_runtime/seen_region.c ] && run_guarded_command "macOS stage3 region" 300 "$OPT_VMEM_KB" \
                clang -O2 -c -I seen_runtime seen_runtime/seen_region.c -o "$NATIVE_REGION" 2>/dev/null || true
            RT_OBJS="$NATIVE_RT"
            [ -f "$NATIVE_REGION" ] && RT_OBJS="$RT_OBJS $NATIVE_REGION"
            if run_guarded_command "macOS stage3 relink" 0 "$OPT_VMEM_KB" \
                clang -O2 -arch arm64 $RELINK_OBJS $RT_OBJS -o "$STAGE3" -lm -lpthread 2>/tmp/relink_link.log; then
                echo -e "${GREEN}    macOS stage3 relink succeeded ($(wc -c < "$STAGE3" | tr -d ' ') bytes).${NC}"
            else
                echo -e "${RED}    macOS stage3 relink failed${NC}"
                cat /tmp/relink_link.log
                exit 1
            fi
            rm -f /tmp/seen_module_*.relink.o "$NATIVE_RT" "$NATIVE_REGION"
        else
            echo -e "${RED}ERROR: macOS stage3 relink failed${NC}"
            exit 1
        fi
    fi

    echo ""
    echo "Stage3 smoke: checking hello-world..."
    if smoke_test_compiler "$STAGE3" "Stage3" "stage3"; then
        echo ""
        echo "Step 3: Verifying bootstrap..."
        if diff "$STAGE2" "$STAGE3" > /dev/null 2>&1; then
            echo -e "${GREEN}Bootstrap verified: Stage2 == Stage3 (identical binaries)!${NC}"
        else
            echo -e "${YELLOW}Note: Stage2 != Stage3 (expected if stage1_frozen is older than source).${NC}"
            echo -e "${GREEN}Stage3 build succeeded — using Stage3 as production compiler.${NC}"
        fi
        VERIFIED="$STAGE3"
    else
        echo -e "${RED}ERROR: Stage3 failed hello-world smoke; repaired frozen Stage2 output is never production-eligible.${NC}" >&2
        exit 1
    fi
else
    # Linux: Attempt S2→S3 bootstrap verification with a timeout.
    # S2 must cold-compile an unmodified Stage3. Repaired frozen Stage2 output
    # is a bootstrap seed only and is never eligible for production install.
    rm -rf .seen_cache/ /tmp/seen_ir_cache/
    rm -f /tmp/seen_module_*.ll /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll
    rm -f /tmp/seen_module_*.opt.status /tmp/seen_module_*.opt.log

    echo ""
    echo "Step 2: Attempting S2→S3 bootstrap verification (Linux)..."
    echo -e "${DIM}Timeout: 30 minutes. Requires an unmodified Stage3 or current-compiler recovery.${NC}"

    S3_MARKER=$(mktemp -d /tmp/seen_stage3_marker.XXXXXX 2>/dev/null || true)
    if run_guarded_command_to_log_with_failure_watch "S2->S3" 1800 "$MAIN_COMPILER_VMEM_KB" /tmp/safe_rebuild_stage3.log \
        env -u SEEN_FORK_SERIALIZER_ROOT_PID \
        SEEN_PACKAGE_CLIENT="$SOURCE_PACKAGE_CLIENT" \
        SEEN_COMPILER_SOURCE_ROOT="$REPO_ROOT" \
        LD_PRELOAD="$FORK_SERIALIZER_SO" \
        SEEN_FORK_SERIALIZER_TARGET="$STAGE2" \
        "$STAGE2" compile "$COMPILER_SOURCE" "$STAGE3" "${STAGE3_COMPILE_FLAGS[@]}" \
        ; then
        rm -rf "$S3_MARKER"
        echo -e "${GREEN}Stage3 build succeeded.${NC}"

        echo ""
        echo "Stage3 smoke: checking hello-world..."
        if smoke_test_compiler "$STAGE3" "Stage3" "stage3"; then
            echo ""
            echo "Step 3: Verifying bootstrap..."
            if diff "$STAGE2" "$STAGE3" > /dev/null 2>&1; then
                echo -e "${GREEN}Bootstrap verified: Stage2 == Stage3 (identical binaries)!${NC}"
            else
                echo -e "${YELLOW}Note: Stage2 != Stage3 (expected if stage1_frozen is older than source).${NC}"
                echo -e "${GREEN}Stage3 build succeeded — using Stage3 as production compiler.${NC}"
            fi
            VERIFIED="$STAGE3"
        else
            echo -e "${YELLOW}Stage3 build completed but failed hello-world smoke; trying current-compiler recovery.${NC}"
            if [ "${SEEN_REQUIRE_STAGE3:-0}" = "1" ]; then
                echo -e "${RED}ERROR: SEEN_REQUIRE_STAGE3=1 and Stage3 smoke failed.${NC}"
                exit 1
            fi
            if recover_with_preserved_production_compiler; then
                echo -e "${GREEN}Using recovered stage3 as production compiler.${NC}"
            else
                echo -e "${RED}ERROR: no unmodified current-compiler Stage3 is available; repaired frozen Stage2 output is never production-eligible.${NC}" >&2
                exit 1
            fi
        fi
    else
        S3_EXIT=$?
        echo -e "${YELLOW}S2→S3 build failed or timed out (exit=$S3_EXIT).${NC}"
        if [ "$S3_EXIT" = "124" ]; then
            echo -e "${YELLOW}Timeout reached — cold-compile hang likely still present.${NC}"
        else
            echo "Check /tmp/safe_rebuild_stage3.log for details."
            tail_log_if_exists /tmp/safe_rebuild_stage3.log 10
        fi
        EXPECTED_STAGE3_MODULES=$(extract_expected_module_count /tmp/safe_rebuild_stage3.log)
        if [ "$IR_RECOVERY_DISABLED" = "1" ]; then
            rm -rf "$S3_MARKER"
            echo -e "${RED}ERROR: Stage3 failed and SEEN_DISABLE_IR_RECOVERY=1 forbids direct IR recovery.${NC}"
            exit "$S3_EXIT"
        fi
        if recover_complete_ll_set_to_compiler "$EXPECTED_STAGE3_MODULES" "$S3_MARKER" "$STAGE3_RECOVERY" "Stage3"; then
            rm -rf "$S3_MARKER"
            echo ""
            echo "Stage3 recovery smoke: checking hello-world..."
            if smoke_test_compiler "$STAGE3_RECOVERY" "Recovered stage3" "stage3_recovery"; then
                echo -e "${GREEN}Using recovered stage3 as production compiler.${NC}"
                VERIFIED="$STAGE3_RECOVERY"
            else
                echo -e "${YELLOW}Recovered stage3 failed hello-world smoke.${NC}"
                if [ "${SEEN_REQUIRE_STAGE3:-0}" = "1" ]; then
                    echo -e "${RED}ERROR: SEEN_REQUIRE_STAGE3=1 and recovered Stage3 smoke failed.${NC}"
                    exit 1
                fi
                echo -e "${RED}ERROR: recovered Stage3 failed smoke; repaired frozen Stage2 output is never production-eligible.${NC}" >&2
                exit 1
            fi
        else
            rm -rf "$S3_MARKER"
            if [ "${SEEN_REQUIRE_STAGE3:-0}" = "1" ]; then
                echo -e "${RED}ERROR: SEEN_REQUIRE_STAGE3=1 and S2→S3 failed.${NC}"
                exit "$S3_EXIT"
            fi
            if recover_with_preserved_production_compiler; then
                echo -e "${GREEN}Using recovered stage3 as production compiler.${NC}"
            else
                echo -e "${RED}ERROR: Stage3 recovery failed; repaired frozen Stage2 output is never production-eligible.${NC}" >&2
                exit "$S3_EXIT"
            fi
        fi
    fi

    rm -rf "$OPT_WRAPPER_DIR"
fi
fi

# A full rebuild must cover the same Stage-1 acceptance surface as verify
# before the selected compiler can replace checkout production artifacts.
run_stage1_acceptance_checks "$VERIFIED" verify || exit 1

# Install production compiler
echo ""
echo "Installing production compiler..."
safe_rebuild_install_checkout_file "$VERIFIED" \
    compiler_seen/target/seen || exit 1
safe_rebuild_install_checkout_file "$PACKAGE_CLIENT_BUILD_OUTPUT" \
    compiler_seen/target/seen-pkg || exit 1
if [ -f "$STAGE2" ] && [ ! -L "$STAGE2" ]; then
    safe_rebuild_install_checkout_file "$STAGE2" stage2_head || exit 1
fi
if [ -f "$STAGE3" ]; then
    safe_rebuild_install_checkout_file "$STAGE3" stage3_head || exit 1
fi
safe_rebuild_remove_checkout_file stage3_recovery_head || exit 1
if [ -f "$STAGE3_RECOVERY" ]; then
    safe_rebuild_install_checkout_file "$STAGE3_RECOVERY" \
        stage3_recovery_head || exit 1
fi

# Also install to target/release/seen (README install path)
safe_rebuild_install_checkout_file "$VERIFIED" target/release/seen || exit 1
safe_rebuild_install_checkout_file "$PACKAGE_CLIENT_BUILD_OUTPUT" \
    target/release/seen-pkg || exit 1
if declare -F seen_build_write_full_release_stamp >/dev/null 2>&1; then
    seen_build_write_full_release_stamp "$REPO_ROOT" "$REPO_ROOT/compiler_seen/target/seen"
fi

# Clean up
rm -f "$STAGE2" "$STAGE3" "$STAGE3_RECOVERY" "$PRESERVED_PROD_BUILDER"
rm -rf .seen_cache/
[ -n "$OPT_WRAPPER_DIR" ] && rm -rf "$OPT_WRAPPER_DIR"

echo ""
echo -e "${GREEN}=== Safe Rebuild Complete ===${NC}"
echo ""
echo "Production compiler updated: compiler_seen/target/seen"
echo "Also installed to: target/release/seen"
[ -f stage3_recovery_head ] && echo "Recovery backup: stage3_recovery_head"
echo "Safe to commit your changes."
