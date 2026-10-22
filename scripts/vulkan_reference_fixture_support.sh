#!/usr/bin/env bash
# Shared software-reference driver and test-only SDK probe setup.
[ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" = 1 ] || {
    echo 'RESOURCE STOP: Vulkan reference fixtures require an attested capped regression' >&2
    exit 126
}
seen_prepare_vulkan_reference() {
    local candidate reference_icd=
    for candidate in /usr/share/vulkan/icd.d/lvp_icd.json /usr/share/vulkan/icd.d/lvp_icd.x86_64.json; do
        if [ -f "$candidate" ] && [ ! -L "$candidate" ]; then reference_icd=$candidate; break; fi
    done
    [ -n "$reference_icd" ] || { echo 'FAIL: explicit Mesa Vulkan reference ICD is unavailable' >&2; return 1; }
    export VK_DRIVER_FILES="$reference_icd" LP_NUM_THREADS=1
    timeout --foreground --kill-after=5s 60s clang -std=c11 -Wall -Wextra -Werror \
        "$ROOT_DIR/tests/production_gpu_runtime/vulkan_abi_oracle.c" -o "$TMP_DIR/vulkan-abi-oracle" || return
    "$TMP_DIR/vulkan-abi-oracle" > "$TMP_DIR/vulkan-abi-oracle.log" || return
    timeout --foreground --kill-after=5s 60s clang -std=c11 -O1 -g -Wall -Wextra -Werror \
        -fPIC -shared "$ROOT_DIR/tests/production_gpu_runtime/vulkan_fault_driver.c" -ldl \
        -o "$TMP_DIR/vulkan-fault-driver.so"
}
seen_run_vulkan_fault() {
    local profile=$1 name=$2 mode=$3 expected=$4
    SEEN_VULKAN_FIXTURE_FAULT="$mode" LD_PRELOAD="$TMP_DIR/vulkan-fault-driver.so" \
        LLVM_PROFILE_FILE="$TMP_DIR/fault-$name-$mode.profraw" \
        run_fixture "$profile" "$name" "$mode" "$expected" || return
    cp -- "$TMP_DIR/$name-$profile.log" "$TMP_DIR/fault-$name-$mode-$profile.log" || return
    rg -Fq 'PASS:' "$TMP_DIR/fault-$name-$mode-$profile.log"
}
seen_run_vulkan_reference() {
    SEEN_VULKAN_FIXTURE_FAULT=none LD_PRELOAD="$TMP_DIR/vulkan-fault-driver.so" \
        run_fixture "$@"
}
