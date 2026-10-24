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
        -o "$TMP_DIR/vulkan-fault-driver.so" || return
    timeout --foreground --kill-after=5s 60s clang -std=c11 -O1 -g -Wall -Wextra -Werror \
        -fno-omit-frame-pointer -fsanitize=address -fPIC -shared \
        "$ROOT_DIR/tests/production_gpu_runtime/vulkan_fault_driver.c" -ldl \
        -o "$TMP_DIR/vulkan-fault-driver-address.so" || return
    FIXTURE_ASAN_PRELOAD_LIBRARY="$TMP_DIR/vulkan-fault-driver-address.so"
}
seen_verify_vulkan_sanitizer_library_rejections() {
    local name=$1 status case_name library digest expected
    [ "${SEEN_VULKAN_ASAN_PREFLIGHT_ROOT:-}" != "$TMP_DIR" ] || return 0
    ln -s -- "$FIXTURE_ASAN_PRELOAD_LIBRARY" "$TMP_DIR/sanitizer-probe-link.so" || return
    for case_name in missing-hash wrong-hash foreign-root symlink not-elf; do
        library=$FIXTURE_ASAN_PRELOAD_LIBRARY
        digest=$(sha256sum -- "$library" | cut -d ' ' -f1)
        case "$case_name" in
            missing-hash) digest=none; expected='requires a SHA-256 identity' ;;
            wrong-hash) digest=$(printf '%064d' 0); expected='SHA-256 mismatch' ;;
            foreign-root) library=/usr/lib/libm.so.6; expected='escaped the exact target root' ;;
            symlink) library="$TMP_DIR/sanitizer-probe-link.so"; expected='escaped the exact target root' ;;
            not-elf) library="$TMP_DIR/vulkan-abi-oracle.log"; digest=$(sha256sum -- "$library" | cut -d ' ' -f1); expected='is not ELF' ;;
        esac
        status=0
        bash "$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh" \
            --target-root "$TMP_DIR" --compile-log "$TMP_DIR/$name-address.log" \
            --fixture-preload "$library" --fixture-preload-sha256 "$digest" \
            -- "$TMP_DIR/$name-address" > "$TMP_DIR/sanitizer-library-rejection-$case_name.log" 2>&1 || status=$?
        [ "$status" = 126 ] && rg -Fq "$expected" "$TMP_DIR/sanitizer-library-rejection-$case_name.log" || {
            echo "FAIL: sanitizer fixture library rejection $case_name (status $status)" >&2; return 1;
        }
    done
    SEEN_VULKAN_ASAN_PREFLIGHT_ROOT=$TMP_DIR
    echo 'PASS: sanitizer fixture library identity root symlink and ELF rejections'
}
seen_run_vulkan_fault() {
    local profile=$1 name=$2 mode=$3 expected=$4
    if [ "$profile" = address ]; then seen_verify_vulkan_sanitizer_library_rejections "$name" || return; fi
    SEEN_VULKAN_FIXTURE_FAULT="$mode" LD_PRELOAD="$TMP_DIR/vulkan-fault-driver.so" \
        LLVM_PROFILE_FILE="$TMP_DIR/fault-$name-$mode.profraw" \
        run_fixture "$profile" "$name" "$mode" "$expected" || return
    cp -- "$TMP_DIR/$name-$profile.log" "$TMP_DIR/fault-$name-$mode-$profile.log" || return
    cp -- "$TMP_DIR/$name-$profile.run.status" "$TMP_DIR/fault-$name-$mode-$profile.status" || return
    rg -Fq 'PASS:' "$TMP_DIR/fault-$name-$mode-$profile.log"
}
seen_run_vulkan_reference() {
    SEEN_VULKAN_FIXTURE_FAULT=none LD_PRELOAD="$TMP_DIR/vulkan-fault-driver.so" \
        run_fixture "$@"
}
