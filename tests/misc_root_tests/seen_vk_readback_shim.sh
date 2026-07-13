#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMP_DIR="$(mktemp -d /tmp/seen_vk_readback.XXXXXX)"

cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

if ! pkg-config --exists vulkan; then
    echo "SKIP: Vulkan development files are unavailable"
    exit 0
fi

cat > "$TMP_DIR/readback_test.c" <<'TEST_EOF'
#include <stdint.h>
#include <stdio.h>

int32_t seen_vk_read_image_rgba8(
    uint64_t physical_device, uint64_t device, uint64_t queue,
    uint64_t command_pool, uint64_t image, int32_t current_layout,
    int32_t width, int32_t height, int32_t format,
    uint8_t* out_rgba, uint64_t out_size, int32_t flip_y);

static int engine_override_called = 0;

void seen_vk_destroy_instance(uint64_t instance) {
    if (instance == 42) engine_override_called = 1;
}

static int expect_result(const char* label, int32_t actual, int32_t expected) {
    if (actual == expected) return 0;
    fprintf(stderr, "%s: expected %d, got %d\n", label, expected, actual);
    return 1;
}

int main(void) {
    uint8_t output[16] = {0};
    int failed = 0;
    failed |= expect_result("null handles",
        seen_vk_read_image_rgba8(0, 0, 0, 0, 0, 0, 1, 1, 37, output, sizeof(output), 0), -3);
    failed |= expect_result("zero width",
        seen_vk_read_image_rgba8(1, 1, 1, 1, 1, 0, 0, 1, 37, output, sizeof(output), 0), -3);
    failed |= expect_result("unsupported format",
        seen_vk_read_image_rgba8(1, 1, 1, 1, 1, 0, 1, 1, 126, output, sizeof(output), 0), -11);
    failed |= expect_result("undersized output",
        seen_vk_read_image_rgba8(1, 1, 1, 1, 1, 0, 4, 4, 37, output, sizeof(output), 0), -1);
    seen_vk_destroy_instance(42);
    failed |= expect_result("strong engine override", engine_override_called, 1);
    if (failed) return 1;
    puts("Vulkan readback shim validation passed");
    return 0;
}
TEST_EOF

: "${SEEN_TEST_VMEM_KB:=16777216}"
(
    ulimit -v "$SEEN_TEST_VMEM_KB"
    cc -std=c11 -Wall -Wextra -Werror -DSEEN_USE_VULKAN \
        $(pkg-config --cflags vulkan) \
        -c "$ROOT_DIR/seen_std/src/platform/linux/shim/seen_platform_shim.c" \
        -o "$TMP_DIR/seen_platform_shim.o"
    if nm -g --defined-only "$TMP_DIR/seen_platform_shim.o" | awk '$2 == "T" && $3 ~ /^seen_vk_/ { print }' | grep -q .; then
        echo "FAIL: Vulkan shim exports must remain weak when engine overrides are linked" >&2
        exit 1
    fi
    cc -std=c11 -Wall -Wextra -Werror \
        "$TMP_DIR/seen_platform_shim.o" \
        "$TMP_DIR/readback_test.c" \
        $(pkg-config --libs vulkan) \
        -o "$TMP_DIR/readback_test"
)
"$TMP_DIR/readback_test"
