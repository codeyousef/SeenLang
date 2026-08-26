#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
HARD_SCOPE="$ROOT_DIR/scripts/run_in_hard_memory_scope.sh"

if [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" != 1 ]; then
    echo "RESOURCE STOP: installed release payload test requires a hard-memory scope" >&2
    exit 126
fi
"$HARD_SCOPE" --label "Installed release payload read-back" --verify-only -- >/dev/null
[ "${SEEN_LOW_MEMORY:-0}" = 1 ] && [ "${SEEN_JOBS:-0}" = 1 ] &&
    [ "${SEEN_OPT_JOBS:-0}" = 1 ] || {
    echo "RESOURCE STOP: installed release payload test requires serial settings" >&2
    exit 126
}

PAYLOAD_VMEM_KB=8388608
if [ "${SEEN_MAIN_VMEM_KB:?}" -lt "$PAYLOAD_VMEM_KB" ]; then
    PAYLOAD_VMEM_KB="$SEEN_MAIN_VMEM_KB"
fi
ulimit -S -v "$PAYLOAD_VMEM_KB" || {
    echo "RESOURCE STOP: could not apply installed-payload VMEM cap" >&2
    exit 126
}
export SEEN_MAIN_VMEM_KB="$PAYLOAD_VMEM_KB"
export SEEN_MEMORY_LIMIT_BYTES=$((PAYLOAD_VMEM_KB * 1024))

COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
PACKAGE_CLIENT="${SEEN_PACKAGE_CLIENT_BIN:-$ROOT_DIR/tools/seen-pkg/bin/seen-pkg}"
WORK_DIR="$(mktemp -d /tmp/release-install-payload.XXXXXX)"
case "$WORK_DIR" in
    /tmp/release-install-payload.*) ;;
    *) echo "ERROR: unsafe installed-payload work path: $WORK_DIR" >&2; exit 1 ;;
esac
cleanup() { rm -rf -- "$WORK_DIR"; }
trap cleanup EXIT

mkdir -p "$WORK_DIR/prefix/bin" "$WORK_DIR/prefix/lib/seen/std" \
    "$WORK_DIR/prefix/lib/seen/runtime" "$WORK_DIR/source"
cp "$COMPILER" "$WORK_DIR/prefix/bin/seen"
cp "$PACKAGE_CLIENT" "$WORK_DIR/prefix/bin/seen-pkg"
cp "$ROOT_DIR/releases/compatibility-manifest.json" \
    "$WORK_DIR/prefix/bin/compatibility-manifest.json"
cp -r "$ROOT_DIR/seen_std/src/"* "$WORK_DIR/prefix/lib/seen/std/"
for runtime_file in "$ROOT_DIR/seen_runtime"/*.{c,h,o,a,sig}; do
    [ -f "$runtime_file" ] && cp "$runtime_file" \
        "$WORK_DIR/prefix/lib/seen/runtime/"
done
cp -r "$ROOT_DIR/seen_runtime/src" "$WORK_DIR/prefix/lib/seen/runtime/"

cmp -s "$WORK_DIR/prefix/bin/compatibility-manifest.json" \
    "$ROOT_DIR/releases/compatibility-manifest.json"
test -f "$WORK_DIR/prefix/lib/seen/std/json/strict.seen"
test -f "$WORK_DIR/prefix/lib/seen/std/crypto/sha256.seen"
"$WORK_DIR/prefix/bin/seen-pkg" --expect-version 0.15.0 version >/dev/null
cp "$ROOT_DIR/tests/misc_root_tests/seen_release_payload_api.seen" \
    "$WORK_DIR/source/main.seen"

(cd "$WORK_DIR/source" && env -u SEEN_COMPILER_SOURCE_ROOT -u SEEN_PACKAGE_CLIENT \
    "$WORK_DIR/prefix/bin/seen" check main.seen)
(cd "$WORK_DIR/source" && env -u SEEN_COMPILER_SOURCE_ROOT -u SEEN_PACKAGE_CLIENT \
    "$WORK_DIR/prefix/bin/seen" compile main.seen payload-smoke \
    --fast --no-cache --no-fork --jobs 1 --opt-jobs 1)
"$WORK_DIR/source/payload-smoke"
echo "PASS: installed layout provides a self-contained Seen 0.15.0 payload"
