#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-json-large-object-release

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ] &&
   [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0" "$@"
fi
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" = 1 ]; then
    bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
    COMPILER_PREFIX=(bash "${SEEN_ATTESTED_COMPILER_RUNNER:?}")
else
    "$ROOT_DIR/scripts/run_in_hard_memory_scope.sh" --verify-only
    COMPILER_PREFIX=()
fi

WORK_DIR="$(mktemp -d "${SEEN_ARTIFACT_ROOT:?}/json-large-object.XXXXXX")"
cleanup() { rm -rf -- "$WORK_DIR"; }
trap cleanup EXIT

awk 'BEGIN {
    printf "{"
    for (i = 0; i < 8192; i++) {
        if (i > 0) printf ","
        printf "\"token_%d\":%d", i, i
    }
    printf "}\n"
}' > "$WORK_DIR/object.json"

timeout --foreground --kill-after=10s 900s \
    "${COMPILER_PREFIX[@]}" "$COMPILER" compile \
    "$ROOT_DIR/seen_std/tests/json/large_object_release.seen" \
    "$WORK_DIR/large-object" --release --lto thin --no-cache --no-fork \
    --jobs 1 --opt-jobs 1
timeout --foreground --kill-after=5s 300s \
    "$WORK_DIR/large-object" "$WORK_DIR/object.json"

grep -Fq '@noinline' "$ROOT_DIR/seen_std/src/json/strict.seen" || {
    echo "FAIL: strict JSON parser lost its release stack boundary" >&2
    exit 1
}
