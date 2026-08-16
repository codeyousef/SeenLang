#!/usr/bin/env bash
# Run a command with all temporary files confined to a unique ignored checkout
# directory. This is the safe entry point for frozen/current Seen compilers
# whose internals still contain absolute /tmp paths.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
ARTIFACT_ROOT_SCRIPT="$SCRIPT_DIR/artifact_root.sh"
KEEP_ON_FAILURE=0
RUN_WORK_ROOT=""
RUN_ARTIFACT_SCOPE=""

usage() {
    cat <<EOF
Usage: $0 <scope> [--keep-on-failure] -- <command> [args...]

Runs the command with TMPDIR and SEEN_ARTIFACT_ROOT set to a unique directory
below <repository>/.seen/agent-tools/<scope>/ and, on Linux, maps that directory
onto /tmp in a private Bubblewrap namespace. Command arguments are preserved.

Options:
  --keep-on-failure  Preserve the unique run directory when the command fails.
  -h, --help         Show this help.
EOF
}

if [ "$#" -eq 0 ]; then
    usage >&2
    exit 64
fi
case "$1" in
    -h|--help)
        usage
        exit 0
        ;;
esac

SCOPE=$1
shift
if [ "${1:-}" = "--keep-on-failure" ]; then
    KEEP_ON_FAILURE=1
    shift
fi
if [ "${1:-}" != "--" ]; then
    echo "ERROR: expected -- before the command" >&2
    usage >&2
    exit 64
fi
shift
if [ "$#" -eq 0 ]; then
    echo "ERROR: missing command after --" >&2
    usage >&2
    exit 64
fi
COMMAND=("$@")

if [ ! -f "$ARTIFACT_ROOT_SCRIPT" ]; then
    echo "ERROR: missing artifact-root helper: $ARTIFACT_ROOT_SCRIPT" >&2
    exit 1
fi
# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_ROOT_SCRIPT"
seen_artifact_root_init "$REPO_ROOT"
RUN_ARTIFACT_SCOPE=$(seen_artifact_scope_init "$SCOPE")
RUN_WORK_ROOT=$(seen_artifact_mktemp_dir "$RUN_ARTIFACT_SCOPE" run)

TMPDIR="$RUN_WORK_ROOT/tool-tmp"
if [ -L "$TMPDIR" ]; then
    echo "ERROR: command temporary directory is a symbolic link: $TMPDIR" >&2
    exit 1
fi
mkdir -p -- "$TMPDIR"
SEEN_ARTIFACT_ROOT="$RUN_WORK_ROOT"
SEEN_PROJECT_ARTIFACT_WRAPPER=1
SEEN_PROJECT_ARTIFACT_NAMESPACE_ACTIVE=1
export TMPDIR SEEN_ARTIFACT_ROOT SEEN_PROJECT_ROOT \
    SEEN_PROJECT_ARTIFACT_WRAPPER SEEN_PROJECT_ARTIFACT_NAMESPACE_ACTIVE

cleanup() {
    local status=$?
    if [ "$status" -ne 0 ] && [ "$KEEP_ON_FAILURE" = "1" ]; then
        printf 'Preserved failed command artifacts: %s\n' "$RUN_WORK_ROOT" >&2
        return "$status"
    fi
    if [ -n "$RUN_WORK_ROOT" ] && [ -n "$RUN_ARTIFACT_SCOPE" ]; then
        case "$RUN_WORK_ROOT" in
            "$RUN_ARTIFACT_SCOPE"/run.*)
                if [ -d "$RUN_WORK_ROOT" ] && [ ! -L "$RUN_WORK_ROOT" ] &&
                    [ "$(dirname -- "$RUN_WORK_ROOT")" = "$RUN_ARTIFACT_SCOPE" ]; then
                    rm -rf -- "$RUN_WORK_ROOT"
                fi
                ;;
        esac
    fi
    return "$status"
}
trap cleanup EXIT

if [ "$(uname -s)" != "Linux" ]; then
    echo "ERROR: project-local mapping of absolute temporary paths is currently supported only on Linux" >&2
    exit 1
fi
if ! command -v bwrap >/dev/null 2>&1; then
    echo "ERROR: Bubblewrap (bwrap) is required; refusing to use the host temporary directory" >&2
    exit 1
fi
if ! bwrap --die-with-parent --bind / / --dev-bind /dev /dev \
    --proc /proc --ro-bind /sys /sys \
    --bind "$RUN_WORK_ROOT" /tmp -- true; then
    echo "ERROR: could not map the project-local artifact directory onto /tmp" >&2
    exit 1
fi

set +e
bwrap --die-with-parent --bind / / --dev-bind /dev /dev \
    --proc /proc --ro-bind /sys /sys \
    --bind "$RUN_WORK_ROOT" /tmp -- \
    "${COMMAND[@]}"
status=$?
set -e
exit "$status"
