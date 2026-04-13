#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="$ROOT_DIR/compiler_seen/target/seen"
TMP_DIR="$(mktemp -d /tmp/seen_import_c_real_headers.XXXXXX)"
STRING_OUT="$TMP_DIR/string.out"
STDIO_OUT="$TMP_DIR/stdio.out"
VULKAN_OUT="$TMP_DIR/vulkan.out"

cleanup() {
    rm -rf "$TMP_DIR"
}

trap cleanup EXIT

STRING_HEADER=""
STDIO_HEADER=""
VULKAN_HEADER=""

for candidate in /usr/include/string.h /usr/local/include/string.h; do
    if [ -f "$candidate" ]; then
        STRING_HEADER="$candidate"
        break
    fi
done

for candidate in /usr/include/stdio.h /usr/local/include/stdio.h; do
    if [ -f "$candidate" ]; then
        STDIO_HEADER="$candidate"
        break
    fi
done

for candidate in /usr/include/vulkan/vulkan.h /usr/local/include/vulkan/vulkan.h; do
    if [ -f "$candidate" ]; then
        VULKAN_HEADER="$candidate"
        break
    fi
done

if [ -z "$STRING_HEADER" ] || [ -z "$STDIO_HEADER" ]; then
    echo "SKIP: system string.h/stdio.h headers not found"
    exit 0
fi

"$COMPILER" import-c "$STRING_HEADER" >"$STRING_OUT"
"$COMPILER" import-c "$STDIO_HEADER" >"$STDIO_OUT"

if [ -n "$VULKAN_HEADER" ]; then
    "$COMPILER" import-c "$VULKAN_HEADER" >"$VULKAN_OUT"
fi

if grep -q '^extern fun prev(' "$STRING_OUT"; then
    echo "FAIL: import-c emitted bogus prev binding for string.h"
    exit 1
fi

if grep -q '^extern fun prev(' "$STDIO_OUT"; then
    echo "FAIL: import-c emitted bogus prev binding for stdio.h"
    exit 1
fi

if [ "$(grep -c '^extern fun memcpy(' "$STRING_OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit memcpy exactly once for string.h"
    exit 1
fi

if [ "$(grep -c '^extern fun fopen(' "$STDIO_OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit fopen exactly once for stdio.h"
    exit 1
fi

if [ -n "$VULKAN_HEADER" ]; then
    if grep -q '^let referenced:' "$VULKAN_OUT"; then
        echo "FAIL: import-c emitted bogus referenced constants for vulkan.h"
        exit 1
    fi

    if [ "$(grep -c '^type VkInstance = Ptr$' "$VULKAN_OUT")" -ne 1 ]; then
        echo "FAIL: import-c should emit VkInstance opaque handle alias for vulkan.h"
        exit 1
    fi

    if [ "$(grep -c '^type VkResult = Int$' "$VULKAN_OUT")" -ne 1 ]; then
        echo "FAIL: import-c should emit VkResult typedef alias for vulkan.h"
        exit 1
    fi

    if [ "$(grep -Fxc 'class VkInstanceCreateInfo {' "$VULKAN_OUT")" -ne 1 ]; then
        echo "FAIL: import-c should emit a repr(C) class for VkInstanceCreateInfo"
        exit 1
    fi

    VK_INSTANCE_CREATE_INFO_BLOCK="$(sed -n '/^class VkInstanceCreateInfo {$/,/^}$/p' "$VULKAN_OUT")"

    if ! printf '%s\n' "$VK_INSTANCE_CREATE_INFO_BLOCK" | grep -Fqx '    var sType: VkStructureType'; then
        echo "FAIL: import-c should preserve VkStructureType in VkInstanceCreateInfo"
        exit 1
    fi

    if ! printf '%s\n' "$VK_INSTANCE_CREATE_INFO_BLOCK" | grep -Fqx '    var pNext: *Void'; then
        echo "FAIL: import-c should emit raw void pointers for VkInstanceCreateInfo.pNext"
        exit 1
    fi

    if [ "$(grep -c '^let VK_ERROR_VALIDATION_FAILED_EXT:' "$VULKAN_OUT")" -ne 1 ]; then
        echo "FAIL: import-c should emit VK_ERROR_VALIDATION_FAILED_EXT exactly once for vulkan.h"
        exit 1
    fi

    if [ "$(grep -c '^extern fun vkCreateInstance(arg0: \*VkInstanceCreateInfo, arg1: \*VkAllocationCallbacks, arg2: \*VkInstance) r: VkResult$' "$VULKAN_OUT")" -ne 1 ]; then
        echo "FAIL: import-c should reuse Vulkan record pointers in vkCreateInstance"
        exit 1
    fi

    if [ "$(grep -c '^extern fun vkDestroyInstance(arg0: VkInstance, arg1: \*VkAllocationCallbacks) r: Void$' "$VULKAN_OUT")" -ne 1 ]; then
        echo "FAIL: import-c should reuse Vulkan record pointers in vkDestroyInstance"
        exit 1
    fi
fi

echo "PASS: import-c handles real system headers without bogus prev/referenced bindings and preserves Vulkan typedefs/records"
