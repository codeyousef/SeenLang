#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
ARTIFACT_HELPER="$ROOT_DIR/scripts/artifact_root.sh"
HARD_SCOPE="$ROOT_DIR/scripts/run_in_hard_memory_scope.sh"
INNER_RELEASE="$ROOT_DIR/scripts/build_and_upload_release.sh"
PROJECT_WRAPPER="$ROOT_DIR/scripts/run_with_project_artifacts.sh"
TIMEOUT_SECS=10800

fail() {
    echo "release-containment: core.001b.invalid: $*" >&2
    exit 126
}

[ "$#" -eq 1 ] || fail "expected exactly one release version"
[ "$(uname -s)" = "Linux" ] ||
    fail "release upload has no supported hard scope on this platform"
[ -f "$ARTIFACT_HELPER" ] && [ ! -L "$ARTIFACT_HELPER" ] ||
    fail "artifact-root helper is missing or unsafe"
[ -x "$HARD_SCOPE" ] && [ ! -L "$HARD_SCOPE" ] ||
    fail "hard-scope entrypoint is missing or unsafe"
[ -x "$INNER_RELEASE" ] && [ ! -L "$INNER_RELEASE" ] ||
    fail "inner release entrypoint is missing or unsafe"
[ -x "$PROJECT_WRAPPER" ] && [ ! -L "$PROJECT_WRAPPER" ] ||
    fail "project-artifact wrapper is missing or unsafe"

# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_HELPER" || fail "could not load artifact-root helper"
seen_artifact_root_init "$ROOT_DIR" || fail "could not validate the artifact root"
scope_root=$(seen_artifact_scope_init release-upload) ||
    fail "could not create the release artifact scope"
RELEASE_TMPDIR=$(seen_artifact_mktemp_dir "$scope_root" run) ||
    fail "could not create the release temporary directory"

cleanup() {
    local status=$?
    case "$RELEASE_TMPDIR" in
        "$scope_root"/run.*)
            if [ -d "$RELEASE_TMPDIR" ] && [ ! -L "$RELEASE_TMPDIR" ] &&
                [ "${RELEASE_TMPDIR%/*}" = "$scope_root" ]; then

                rm -rf -- "$RELEASE_TMPDIR" || return 1
            fi
            ;;
        *) return 1 ;;
    esac
    return "$status"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

status=0
env TMPDIR="$RELEASE_TMPDIR" \
    "$HARD_SCOPE" --label "signed release upload" --timeout-secs "$TIMEOUT_SECS" -- \
    env SEEN_CI_CONTAINMENT_IN_SCOPE=1 SEEN_RELEASE_CONTAINMENT_IN_SCOPE=1 \
        SEEN_RELEASE_PROJECT_WRAPPER="$PROJECT_WRAPPER" \
        SEEN_JOBS=1 SEEN_OPT_JOBS=1 SEEN_PACKAGE_JOBS=1 SEEN_NO_FORK=1 \
        "$INNER_RELEASE" "$1" || status=$?
if [ "$status" -ne 0 ]; then
    echo "release-containment: core.001b.invalid: contained release failed (status $status)" >&2
fi
exit "$status"
