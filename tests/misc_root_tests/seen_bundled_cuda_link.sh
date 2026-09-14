#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
if [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != 1 ] ||
   [ "${SEEN_LOW_MEMORY:-0}" != 1 ] ||
   [ "${SEEN_JOBS:-0}" != 1 ] || [ "${SEEN_OPT_JOBS:-0}" != 1 ]; then
    echo "FAIL: bundled CUDA link test requires verified serial hard scope" >&2
    exit 126
fi
"$ROOT_DIR/scripts/run_in_hard_memory_scope.sh" --verify-only

SEEN_BIN=${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}
TEST_ROOT="${SEEN_ARTIFACT_ROOT:?}/fel-1579"
mkdir -p "$TEST_ROOT"

run_case() {
    local mode=$1
    local output=$2
    shift 2
    (
        cd "$TEST_ROOT/bundled"
        "$SEEN_BIN" compile src/main.seen "$output" "$@" \
            --target-cpu=x86-64 --no-cache --jobs 1 --opt-jobs 1 --no-fork
    )
    "$output"
    readelf -d "$output" | grep -Fq 'libseen_cuda.so.1'
}

cp -R "$ROOT_DIR/tests/fixtures/fel-1579/bundled" "$TEST_ROOT/bundled"
run_case fast "$TEST_ROOT/bundled-fast"
run_case release "$TEST_ROOT/bundled-release" --release --lto=thin
find "$SEEN_ARTIFACT_ROOT/native-dependencies/seen_cuda" -type f \
    -name 'libseen_cuda.so.sig' -print -quit | grep -q . || {
    echo "FAIL: bundled CUDA cache has no digest signature" >&2
    exit 1
}

cp -R "$ROOT_DIR/tests/fixtures/fel-1579/missing" "$TEST_ROOT/missing"
if (
    cd "$TEST_ROOT/missing"
    env PATH=/usr/bin:/bin "$SEEN_BIN" compile src/main.seen \
        "$TEST_ROOT/missing-output" \
        --release --lto=thin --target-cpu=x86-64 --no-cache \
        --jobs 1 --opt-jobs 1 --no-fork
) >"$TEST_ROOT/missing.log" 2>&1; then
    echo "FAIL: CUDA call linked without explicit bundled dependency" >&2
    exit 1
fi
test ! -e "$TEST_ROOT/missing-output"
grep -Fq '[seen.cuda.link] packaged CUDA runtime is not enabled' \
    "$TEST_ROOT/missing.log"

make_staged_root() {
    local destination=$1
    mkdir -p "$destination/compiler_seen" "$destination/releases"
    ln -s "$ROOT_DIR/compiler_seen/src" "$destination/compiler_seen/src"
    ln -s "$ROOT_DIR/seen_std" "$destination/seen_std"
    cp -R "$ROOT_DIR/seen_runtime" "$destination/seen_runtime"
    cp "$ROOT_DIR/releases/compatibility-manifest.json" \
        "$destination/releases/compatibility-manifest.json"
}

make_staged_root "$TEST_ROOT/incompatible-root"
sed -i 's/SEEN_CUDA_ABI_VERSION 1u/SEEN_CUDA_ABI_VERSION 2u/' \
    "$TEST_ROOT/incompatible-root/seen_runtime/cuda/include/seen_cuda.h"
if (
    cd "$TEST_ROOT/bundled"
    env SEEN_COMPILER_SOURCE_ROOT="$TEST_ROOT/incompatible-root" \
        "$SEEN_BIN" compile src/main.seen "$TEST_ROOT/incompatible-output" \
        --target-cpu=x86-64 --no-cache --jobs 1 --opt-jobs 1 --no-fork
) >"$TEST_ROOT/incompatible.log" 2>&1; then
    echo "FAIL: incompatible packaged CUDA ABI was accepted" >&2
    exit 1
fi
test ! -e "$TEST_ROOT/incompatible-output"
grep -Fq 'packaged seen_cuda runtime is missing or has an incompatible ABI' \
    "$TEST_ROOT/incompatible.log"
! grep -Fq 'Build succeeded' "$TEST_ROOT/incompatible.log"

make_staged_root "$TEST_ROOT/missing-root"
rm -f -- "$TEST_ROOT/missing-root/seen_runtime/cuda/src/seen_cuda.cu"
if (
    cd "$TEST_ROOT/bundled"
    env SEEN_COMPILER_SOURCE_ROOT="$TEST_ROOT/missing-root" \
        "$SEEN_BIN" compile src/main.seen "$TEST_ROOT/missing-runtime-output" \
        --target-cpu=x86-64 --no-cache --jobs 1 --opt-jobs 1 --no-fork
) >"$TEST_ROOT/missing-runtime.log" 2>&1; then
    echo "FAIL: missing packaged CUDA source was accepted" >&2
    exit 1
fi
test ! -e "$TEST_ROOT/missing-runtime-output"
grep -Fq 'packaged seen_cuda runtime is missing or has an incompatible ABI' \
    "$TEST_ROOT/missing-runtime.log"
! grep -Fq 'Build succeeded' "$TEST_ROOT/missing-runtime.log"

echo "PASS: explicit bundled CUDA dependency links fast and release executables"
