#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
PROJECT_WRAPPER="$ROOT_DIR/scripts/run_with_project_artifacts.sh"
HARD_SCOPE="$ROOT_DIR/scripts/run_in_hard_memory_scope.sh"
CHECKER="$ROOT_DIR/scripts/check_x86_executable_baseline.sh"
FIXTURE="$ROOT_DIR/tests/fixtures/release-baseline/instruction_probe.c"

if [ "${SEEN_PROJECT_ARTIFACT_NAMESPACE_ACTIVE:-0}" != 1 ]; then
    exec "$PROJECT_WRAPPER" executable-baseline-contract -- bash "$0" "$@"
fi
if [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != 1 ]; then
    exec "$HARD_SCOPE" --label "executable CPU baseline contract" \
        --timeout-secs 300 -- bash "$0" "$@"
fi
"$HARD_SCOPE" --verify-only

BUILD_DIR="${SEEN_ARTIFACT_ROOT:?}/executable-baseline-contract"
mkdir -p "$BUILD_DIR"
clang -O0 "$FIXTURE" -o "$BUILD_DIR/portable"
clang -O0 -mavx -DSEEN_PROBE_AVX "$FIXTURE" -o "$BUILD_DIR/avx"
clang -O0 -mavx512f -DSEEN_PROBE_AVX512 "$FIXTURE" \
    -o "$BUILD_DIR/avx512"

bash "$CHECKER" x86-64 "$BUILD_DIR/portable"
if bash "$CHECKER" x86-64 "$BUILD_DIR/avx" >/dev/null 2>&1; then
    echo "FAIL: x86-64 audit accepted an AVX executable" >&2
    exit 1
fi
bash "$CHECKER" x86-64-v3 "$BUILD_DIR/avx"
if bash "$CHECKER" x86-64-v3 "$BUILD_DIR/avx512" >/dev/null 2>&1; then
    echo "FAIL: x86-64-v3 audit accepted an AVX-512 executable" >&2
    exit 1
fi

echo "PASS: executable CPU baseline positive and negative contracts"
