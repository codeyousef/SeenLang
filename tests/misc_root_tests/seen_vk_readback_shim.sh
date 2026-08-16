#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-vk-readback-shim
if [ "${SEEN_CAPPED_PLATFORM_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" --platform "$SCOPE" -- bash "$0" "$@"
fi
bash "$CAPPED_ENTRY" --verify-platform-active "$SCOPE"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-vk-readback.XXXXXX")"

cleanup() {
    case "$TMP_DIR" in
        "$SEEN_ARTIFACT_ROOT"/seen-vk-readback.*)
            [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                [ "$(dirname -- "$TMP_DIR")" = "$SEEN_ARTIFACT_ROOT" ] || return 1
            rm -rf -- "$TMP_DIR"
            ;;
        *) return 1 ;;
    esac
}
trap cleanup EXIT

run_helper_capped() {
    (
        if ! ulimit -S -v "$SEEN_OPT_VMEM_KB" 2>/dev/null; then
            echo "RESOURCE STOP: could not apply Vulkan helper memory cap" >&2
            exit 126
        fi
        active_vmem=$(ulimit -S -v 2>/dev/null || true)
        case "$active_vmem" in
            ''|*[!0-9]*)
                echo "RESOURCE STOP: could not read back Vulkan helper memory cap" >&2
                exit 126
                ;;
        esac
        [ "$active_vmem" -le "$SEEN_OPT_VMEM_KB" ] || {
            echo "RESOURCE STOP: Vulkan helper cap exceeds request" >&2
            exit 126
        }
        "$@"
    )
}

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

(
    run_helper_capped cc -std=c11 -Wall -Wextra -Werror -DSEEN_USE_VULKAN \
        $(pkg-config --cflags vulkan) \
        -c "$ROOT_DIR/seen_std/src/platform/linux/shim/seen_platform_shim.c" \
        -o "$TMP_DIR/seen_platform_shim.o"
    if nm -g --defined-only "$TMP_DIR/seen_platform_shim.o" | awk '$2 == "T" && $3 ~ /^seen_vk_/ { print }' | grep -q .; then
        echo "FAIL: Vulkan shim exports must remain weak when engine overrides are linked" >&2
        exit 1
    fi
    run_helper_capped cc -std=c11 -Wall -Wextra -Werror \
        "$TMP_DIR/seen_platform_shim.o" \
        "$TMP_DIR/readback_test.c" \
        $(pkg-config --libs vulkan) \
        -o "$TMP_DIR/readback_test"
)
"$TMP_DIR/readback_test"
