#include "seen_runtime.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

// Internal slab ABI exercised directly to certify reserve accounting.
void *seen_pool_alloc(int64_t size);
void seen_pool_free(void *ptr, int64_t size);
SeenString seen_float_to_string(double value);

int main(void) {
    const int64_t slab_bytes = 1048576 + 24;
    const int64_t initial = seen_memory_used_bytes();
    const int64_t failures = seen_memory_allocation_failures();
    void *first = seen_pool_alloc(1);
    if (!first || seen_memory_used_bytes() != initial + 8 ||
        seen_memory_reserved_bytes() != slab_bytes - 8) return 1;
    seen_pool_free(first, 1);
    if (seen_memory_used_bytes() != initial ||
        seen_memory_reserved_bytes() != slab_bytes) return 2;

    for (int i = 0; i < 1000; i++) {
        void *slot = seen_pool_alloc(1);
        if (!slot || seen_memory_used_bytes() != initial + 8) return 3;
        seen_pool_free(slot, 1);
        if (seen_memory_used_bytes() != initial ||
            seen_memory_reserved_bytes() != slab_bytes) return 4;
    }
    // The committed reserve still consumes the bounded physical budget.
    seen_memory_set_limit_bytes(slab_bytes);
    if (seen_memory_remaining_bytes() != 0 ||
        seen_memory_allocation_failures() != failures) return 5;
    void *recycled = seen_pool_alloc(1);
    if (!recycled || seen_memory_used_bytes() != initial + 8) return 6;
    seen_pool_free(recycled, 1);
    if (seen_memory_used_bytes() != initial) return 7;
    seen_memory_set_limit_bytes(0);

    // Owned formatted strings must be returned in the size class implied by
    // their actual length, not a fixed scratch-buffer class.
    for (int i = 0; i < 100; i++) {
        SeenString signed_text = seen_int_to_string(i - 64);
        SeenString unsigned_text = seen_uint_to_string((uint64_t)i);
        SeenString float_text = seen_float_to_string((double)i / 4.0);
        SeenString unicode_text = seen_char_to_owned_string(0x1f600);
        seen_string_release_owned(signed_text);
        seen_string_release_owned(unsigned_text);
        seen_string_release_owned(float_text);
        seen_string_release_owned(unicode_text);
        if (seen_memory_used_bytes() != initial) return 12;
    }

    // Ten size classes and two overflow slabs reproduce twelve retained
    // allocations of 1,048,576 + 24 bytes without treating them as live.
    for (int size = 1; size <= 80; size += 8) {
        void *slot = seen_pool_alloc(size);
        if (!slot) return 8;
        seen_pool_free(slot, size);
    }
    for (int size = 1; size <= 9; size += 8) {
        const size_t count = (size_t)1048576 / (size == 1 ? 8 : 16) + 1;
        void **slots = malloc(count * sizeof(void *));
        if (!slots) return 9;
        for (size_t i = 0; i < count; i++) {
            slots[i] = seen_pool_alloc(size);
            if (!slots[i]) return 10;
        }
        for (size_t i = 0; i < count; i++) seen_pool_free(slots[i], size);
        free(slots);
    }
    if (seen_memory_used_bytes() != initial ||
        seen_memory_reserved_bytes() != 12 * slab_bytes ||
        seen_memory_allocation_failures() != failures) return 11;
    puts("PASS: pooled live usage, recyclable reserve, physical budget");
    return 0;
}
