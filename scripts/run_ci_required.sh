#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
ARTIFACT_HELPER="$ROOT_DIR/scripts/artifact_root.sh"
HARD_SCOPE="$ROOT_DIR/scripts/run_in_hard_memory_scope.sh"
INNER_GATE="$ROOT_DIR/scripts/ci_required.sh"
TIMEOUT_SECS=20700
CI_PHASE=all
CI_GROUP=""

fail() {
    echo "ci-containment: core.001b.invalid: $*" >&2
    exit 126
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --phase)
            [ "$#" -ge 2 ] || fail "--phase requires a value"
            CI_PHASE=$2
            shift 2
            ;;
        --group)
            [ "$#" -ge 2 ] || fail "--group requires a value"
            CI_GROUP=$2
            shift 2
            ;;
        *) fail "unknown required CI option: $1" ;;
    esac
done
case "$CI_PHASE" in
    all) ;;
    bootstrap) TIMEOUT_SECS=7200 ;;
    acceptance)
        TIMEOUT_SECS=11700
        case "$CI_GROUP" in
            core|io-simd|gpu-emitter|gpu-runtime-async) ;;
            *) fail "acceptance requires one complete named group" ;;
        esac
        export SEEN_TIER_TIMEOUT_SECS="$TIMEOUT_SECS"
        ;;
    finalize) TIMEOUT_SECS=1800 ;;
    *) fail "unknown required CI phase: $CI_PHASE" ;;
esac
[ "$CI_PHASE" = acceptance ] || [ -z "$CI_GROUP" ] ||
    fail "--group is only valid for the acceptance phase"

[ "$(uname -s)" = "Linux" ] ||
    fail "required CI has no supported hard scope on this platform"
[ -f "$ARTIFACT_HELPER" ] && [ ! -L "$ARTIFACT_HELPER" ] ||
    fail "artifact-root helper is missing or unsafe"
[ -x "$HARD_SCOPE" ] && [ ! -L "$HARD_SCOPE" ] ||
    fail "hard-scope entrypoint is missing or unsafe"
[ -x "$INNER_GATE" ] && [ ! -L "$INNER_GATE" ] ||
    fail "inner required gate is missing or unsafe"

# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_HELPER" || fail "could not load artifact-root helper"
seen_artifact_root_init "$ROOT_DIR" || fail "could not validate the artifact root"
scope_root=$(seen_artifact_scope_init ci-required) ||
    fail "could not create the CI artifact scope"
CI_TMPDIR=$(seen_artifact_mktemp_dir "$scope_root" run) ||
    fail "could not create the CI temporary directory"

cleanup() {
    local status=$?
    case "$CI_TMPDIR" in
        "$scope_root"/run.*)
            if [ -d "$CI_TMPDIR" ] && [ ! -L "$CI_TMPDIR" ] &&
                [ "${CI_TMPDIR%/*}" = "$scope_root" ]; then

                rm -rf -- "$CI_TMPDIR" || return 1
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
env TMPDIR="$CI_TMPDIR" \
    "$HARD_SCOPE" --label "required CI" --timeout-secs "$TIMEOUT_SECS" -- \
    env SEEN_CI_CONTAINMENT_IN_SCOPE=1 SEEN_CI_PHASE="$CI_PHASE" \
        SEEN_CI_ACCEPTANCE_GROUP="$CI_GROUP" "$INNER_GATE" || status=$?
if [ "$status" -ne 0 ]; then
    echo "ci-containment: core.001b.invalid: contained required gates failed (status $status)" >&2
fi
exit "$status"
