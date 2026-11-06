#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-runtime-g
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-runtime-g.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-runtime-g
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
source "$ROOT_DIR/scripts/vulkan_reference_fixture_support.sh"
seen_prepare_vulkan_reference
export SEEN_VULKAN_FIXTURE_COPY_TRACE=1 SEEN_VULKAN_FIXTURE_STREAM_TRACE=1
for profile in plain address; do
    flags=(); destination="$TMP_DIR/vulkan-fault-driver.so"
    if [ "$profile" = address ]; then flags=(-fno-omit-frame-pointer -fsanitize=address); destination="$TMP_DIR/vulkan-fault-driver-address.so"; fi
    timeout --foreground --kill-after=5s 60s clang -std=c11 -O1 -g -Wall -Wextra -Werror \
        -fPIC -shared "${flags[@]}" "$ROOT_DIR/tests/production_gpu_runtime/vulkan_recovery_fault_driver.c" -ldl -o "$destination"
done
run_compiler gpu emit "$ROOT_DIR/tests/production_gpu_runtime/vulkan_foundation_kernel.seen" \
    --gpu-backend spirv --gpu-target vulkan1.2 --output "$TMP_DIR/recovery.spv" \
    --reflection "$TMP_DIR/recovery.json" > "$TMP_DIR/emit.log" 2>&1
spirv-val --target-env vulkan1.2 "$TMP_DIR/recovery.spv"
run_recovery() {
    local profile=$1 mode=$2 status=$3
    seen_run_vulkan_reference "$profile" recovery "$TMP_DIR/recovery.spv" "$TMP_DIR/recovery.json" software-reference "$mode" "$status"
    cp -- "$TMP_DIR/00-active.log" "$TMP_DIR/recovery-$profile-$mode-$status.log"
    cp -- "$TMP_DIR/recovery-$profile.run.status" "$TMP_DIR/recovery-$profile-$mode-$status.status"
}
for profile in fast release undefined address; do
    compile_fixture "$profile" fallback-policy "$ROOT_DIR/tests/production_gpu_runtime/fallback_contract.seen"
    run_fixture "$profile" fallback-policy
    readelf -d "$TMP_DIR/fallback-policy-$profile" > "$TMP_DIR/fallback-policy-$profile.libraries"
    if rg -i 'NEEDED.*(vulkan|cuda|cudart|cublas|amdhip|metal)' "$TMP_DIR/fallback-policy-$profile.libraries"; then
        echo 'FAIL: caller fallback policy implicitly linked a GPU SDK' >&2; exit 1
    fi
    compile_vulkan_fixture "$profile" recovery "$ROOT_DIR/tests/production_gpu_runtime/vulkan_context_recovery_contract.seen"
    run_recovery "$profile" shared-loss -4
    run_recovery "$profile" g-memory-oom -2
    run_recovery "$profile" g-pipeline-partial-oom -1
    for status in -12 -1000069000 -1000161000; do run_recovery "$profile" g-descriptor-pool-oom "$status"; done
    run_recovery "$profile" g-descriptor-set-oom -1000069000
    run_recovery "$profile" g-object-limit -10
    run_recovery "$profile" g-staging-second-memory -2
    compile_vulkan_fixture "$profile" fallback-native "$ROOT_DIR/tests/production_gpu_runtime/vulkan_fallback_contract.seen"
    for mode in oom-disabled oom-enabled lost-disabled lost-enabled lost-cpu-error oom-deadline; do
        fault=copy-submit-oom
        case "$mode" in lost-*) fault=copy-submit-lost ;; esac
        SEEN_VULKAN_FIXTURE_FAULT="$fault" LD_PRELOAD="$TMP_DIR/vulkan-fault-driver.so" \
            run_fixture "$profile" fallback-native "$TMP_DIR/recovery.spv" "$TMP_DIR/recovery.json" software-reference "$mode"
        cp -- "$TMP_DIR/00-active.log" "$TMP_DIR/fallback-native-$profile-$mode.log"
        cp -- "$TMP_DIR/fallback-native-$profile.run.status" "$TMP_DIR/fallback-native-$profile-$mode.status"
    done
done
echo 'PASS: Runtime-G typed native failures, shared-context loss isolation, actual explicit CPU fallback and deterministic shutdown (software-reference)'
