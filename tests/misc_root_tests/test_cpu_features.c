// C test for CPU feature detection and arena allocator
// Compile: clang -O2 -I seen_runtime seen_runtime/seen_runtime.c seen_runtime/seen_region.c tests/misc_root_tests/test_cpu_features.c -o /tmp/test_cpu_features -lm
// Run: /tmp/test_cpu_features

#include "seen_runtime.h"
#include <stdio.h>
#include <string.h>

int main() {
    printf("=== CPU Feature Detection Test (C) ===\n");

    // Test CPU detection
    seen_cpu_detect();

    SeenString sse2 = {4, "sse2"};
    SeenString sse42 = {6, "sse4.2"};
    SeenString avx2 = {4, "avx2"};
    SeenString avx512f = {7, "avx512f"};
    SeenString neon = {4, "neon"};
    SeenString apx = {3, "apx"};
    SeenString avx10 = {5, "avx10"};
    SeenString unknown = {7, "unknown"};

    printf("SSE2:     %ld\n", (long)seen_cpu_has_feature(sse2));
    printf("SSE4.2:   %ld\n", (long)seen_cpu_has_feature(sse42));
    printf("AVX2:     %ld\n", (long)seen_cpu_has_feature(avx2));
    printf("AVX-512F: %ld\n", (long)seen_cpu_has_feature(avx512f));
    printf("NEON:     %ld\n", (long)seen_cpu_has_feature(neon));
    printf("APX:      %ld\n", (long)seen_cpu_has_feature(apx));
    printf("AVX10:    %ld\n", (long)seen_cpu_has_feature(avx10));

    int64_t tier = seen_cpu_simd_tier();
    printf("SIMD Tier: %ld\n", (long)tier);

    // Verify tier is reasonable
    if (tier < 0 || tier > 5) {
        printf("FAIL: tier out of range\n");
        return 1;
    }

    // Unknown feature should return 0
    if (seen_cpu_has_feature(unknown) != 0) {
        printf("FAIL: unknown feature should return 0\n");
        return 1;
    }

    printf("\n=== Arena Allocator Test (C) ===\n");

    // Test arena
    void* arena = seen_arena_new(4096);
    if (!arena) {
        printf("FAIL: arena creation failed\n");
        return 1;
    }
    printf("Arena created (4096 bytes)\n");

    // Check initial state
    if (seen_arena_used(arena) != 0) {
        printf("FAIL: initial used should be 0\n");
        return 1;
    }
    if (seen_arena_remaining(arena) != 4096) {
        printf("FAIL: initial remaining should be 4096\n");
        return 1;
    }

    // Allocate
    int64_t idx1 = seen_arena_alloc(arena, 64);
    int64_t idx2 = seen_arena_alloc(arena, 128);
    printf("Alloc 1: index=%ld\n", (long)idx1);
    printf("Alloc 2: index=%ld\n", (long)idx2);

    if (idx1 < 0 || idx2 < 0) {
        printf("FAIL: allocation returned invalid index\n");
        return 1;
    }
    if (idx2 <= idx1) {
        printf("FAIL: second index should be > first\n");
        return 1;
    }

    // Get pointers
    void* ptr1 = seen_arena_get(arena, idx1);
    void* ptr2 = seen_arena_get(arena, idx2);
    if (!ptr1 || !ptr2) {
        printf("FAIL: get returned null\n");
        return 1;
    }

    printf("Used: %ld bytes\n", (long)seen_arena_used(arena));

    // Reset
    seen_arena_reset(arena);
    if (seen_arena_used(arena) != 0) {
        printf("FAIL: used should be 0 after reset\n");
        return 1;
    }
    printf("Reset OK\n");

    // Alloc after reset
    int64_t idx3 = seen_arena_alloc(arena, 32);
    if (idx3 < 0) {
        printf("FAIL: alloc after reset failed\n");
        return 1;
    }
    printf("Alloc after reset: index=%ld\n", (long)idx3);

    // Free
    seen_arena_free(arena);
    printf("Arena freed\n");

    printf("\n=== All C tests passed ===\n");
    return 0;
}
