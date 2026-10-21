#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-spirv-c
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-spirv-c.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-spirv-c
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
python3 "$ROOT_DIR/scripts/check_gpu_spirv_toolchain.py" > "$TMP_DIR/toolchain.json"
for profile in fast release undefined address; do
    compile_case "$profile" owned-arguments "$ROOT_DIR/tests/compiler_regressions/owned_cli_arguments.seen"
    compile_case "$profile" array-fill "$ROOT_DIR/tests/compiler_regressions/fixed_width_array_fill.seen"
    rg -Fq 'PASS: fixed-width filled arrays preserve zeroing strides and ownership' "$TMP_DIR/array-fill-$profile.log"
    compile_case "$profile" sha256-vectors "$ROOT_DIR/tests/fixtures/sha256_finalize_ownership.seen"
    rg -Fq 'PASS: SHA-256 allocator baseline' "$TMP_DIR/sha256-vectors-$profile.log"
    compile_case "$profile" artifact "$ROOT_DIR/tests/spir_v_vulkan_lowering/artifact_contract.seen" "$ROOT_DIR/tests/spir_v_vulkan_lowering/packed_happy.seen"
    rg -Fq 'PASS: GPU-002C artifact hashes manifests mutation rejection and allocation baseline' "$TMP_DIR/artifact-$profile.log"
done
for invalid in negative overflow; do
    status=0
    timeout --foreground --kill-after=5s 30s "$TMP_DIR/array-fill-fast" "$invalid" > "$TMP_DIR/array-fill-$invalid.log" 2>&1 || status=$?
    if [ "$status" -eq 0 ]; then echo "FAIL: invalid filled array length accepted: $invalid" >&2; exit 1; fi
    case "$status" in 124|125|126|127|137|143) exit "$status" ;; esac
    rg -Fq 'PANIC: index out of bounds:' "$TMP_DIR/array-fill-$invalid.log"
done
mkdir "$TMP_DIR/relocated"
cp "$ROOT_DIR/tests/spir_v_vulkan_lowering/packed_happy.seen" "$TMP_DIR/relocated/source.seen"
for target in vulkan1.2 vulkan1.3; do
    for copy in original relocated; do
        source="$ROOT_DIR/tests/spir_v_vulkan_lowering/packed_happy.seen"
        if [ "$copy" = relocated ]; then source="$TMP_DIR/relocated/source.seen"; fi
        run_compiler gpu emit "$source" --gpu-backend spirv --gpu-target "$target" \
            --module-id spirv-artifact --artifact-info --output "$TMP_DIR/$copy-$target.spv" \
            --reflection "$TMP_DIR/$copy-$target.json" > "$TMP_DIR/$copy-$target.log" 2>&1
    done
done
python3 "$ROOT_DIR/scripts/certify_gpu_spirv.py" "$TMP_DIR"
