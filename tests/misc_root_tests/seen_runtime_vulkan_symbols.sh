#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
TMP_DIR="/tmp/seen_runtime_vulkan_symbols"
DEFAULT_OBJ="$TMP_DIR/seen_runtime_default.o"
BOOTSTRAP_OBJ="$TMP_DIR/seen_runtime_bootstrap_vulkan.o"

rm -rf "$TMP_DIR"
mkdir -p "$TMP_DIR"

clang -O2 -c -I "$ROOT_DIR/seen_runtime" "$ROOT_DIR/seen_runtime/seen_runtime.c" -o "$DEFAULT_OBJ"

if nm -g "$DEFAULT_OBJ" | grep -q ' vkDestroyInstance$'; then
    echo "FAIL: default runtime object still exports raw Vulkan stub vkDestroyInstance"
    exit 1
fi

if nm -g "$DEFAULT_OBJ" | grep -q ' vkDestroyRenderPass$'; then
    echo "FAIL: default runtime object still exports raw Vulkan stub vkDestroyRenderPass"
    exit 1
fi

clang -O2 -DSEEN_ENABLE_BOOTSTRAP_RAW_VULKAN_STUBS -c -I "$ROOT_DIR/seen_runtime" "$ROOT_DIR/seen_runtime/seen_runtime.c" -o "$BOOTSTRAP_OBJ"

if ! nm -g "$BOOTSTRAP_OBJ" | grep -q ' vkDestroyInstance$'; then
    echo "FAIL: bootstrap runtime object should export raw Vulkan stubs when explicitly enabled"
    exit 1
fi

echo "PASS: runtime Vulkan symbols are hidden by default and opt-in for bootstrap builds"
