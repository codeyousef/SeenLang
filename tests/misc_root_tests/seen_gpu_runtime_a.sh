#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-runtime-a
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-runtime-a.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-runtime-a
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
fixtures=(backend error limits device_info discovery)
for profile in fast release undefined address; do
    for name in "${fixtures[@]}"; do
        compile_case "$profile" "$name" "$ROOT_DIR/tests/production_gpu_runtime/${name}_contract.seen"
        rg -Fq 'PASS: GPU-006A' "$TMP_DIR/$name-$profile.log"
    done
done
# Metadata and CPU-default imports must not discover or link an accelerator SDK.
readelf -d "$TMP_DIR/backend-fast" > "$TMP_DIR/cpu-default-libraries.txt"
if rg -i 'NEEDED.*(cuda|cudart|vulkan|amdhip|metal)' "$TMP_DIR/cpu-default-libraries.txt"; then
    echo 'FAIL: CPU-default metadata import linked an accelerator SDK' >&2
    exit 1
fi
FIXTURE_COMPILE_FLAGS=(--coverage)
coverage=()
for name in "${fixtures[@]}"; do
    export LLVM_PROFILE_FILE="$TMP_DIR/$name.profraw"
    compile_case fast "coverage-$name" "$ROOT_DIR/tests/production_gpu_runtime/${name}_contract.seen"
    coverage+=(--fixture "$TMP_DIR/coverage-$name-fast" "$TMP_DIR/$name.profraw")
done
source "$ROOT_DIR/scripts/source_coverage_fixture_support.sh"
seen_verify_source_coverage "$TMP_DIR/foundations" "${coverage[@]}" \
    --source seen_std/src/compute/backend.seen \
    --source seen_std/src/compute/error.seen \
    --source seen_std/src/compute/limits.seen \
    --source seen_std/src/compute/capabilities.seen \
    --source seen_std/src/compute/device_info.seen \
    --source seen_std/src/compute/discovery.seen
echo 'PASS: bounded GPU discovery metadata and ownership; driver contexts and hardware execution unverified'
