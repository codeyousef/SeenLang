#include "seen_runtime.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#if defined(__linux__)
#include <signal.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <unistd.h>
#endif

// Internal slab ABI exercised directly to certify reserve accounting.
void *seen_pool_alloc(int64_t size);
void seen_pool_free(void *ptr, int64_t size);
SeenString seen_float_to_string(double value);

// Current internal slab has base, used, capacity and next. The capacity field
// was added for bounded small-budget slabs; do not retain the old 24-byte ABI.
struct PoolSlabHeaderContract {
    char *base;
    size_t used;
    size_t capacity;
    void *next;
};

#if defined(__linux__)
static int rejected_release(void *pointer, int64_t size) {
    pid_t child = fork();
    if (child < 0) return 0;
    if (child == 0) {
        struct rlimit no_core = {0, 0};
        if (setrlimit(RLIMIT_CORE, &no_core) != 0) _exit(101);
        seen_pool_free(pointer, size);
        _exit(100);
    }
    int status = 0;
    return waitpid(child, &status, 0) == child &&
        WIFSIGNALED(status) && WTERMSIG(status) == SIGABRT;
}
#endif

int main(void) {
    const int64_t slab_bytes = 1048576 + (int64_t)sizeof(struct PoolSlabHeaderContract);
    const int64_t initial = seen_memory_used_bytes();
    const int64_t failures = seen_memory_allocation_failures();
    const int64_t initial_reserve = seen_memory_reserved_bytes();
    // Native Result/Vec headers use budgeted malloc, not pool slots. Generic
    // class cleanup must release that allocation rather than enroll it in an
    // unrelated freelist and cancel raw live usage with a negative pool count.
    const int64_t raw_sizes[] = {8, 16, 24, 32, 64, 72, 80, 88};
    for (size_t i = 0; i < sizeof(raw_sizes) / sizeof(raw_sizes[0]); ++i) {
        for (int repeat = 0; repeat < 1000; ++repeat) {
            void *raw = seen_try_malloc(raw_sizes[i]);
            if (!raw) return 13;
            seen_pool_free(raw, raw_sizes[i]);
            if (seen_memory_used_bytes() != initial ||
                seen_memory_reserved_bytes() != initial_reserve) {
                fputs("FAIL: malloc-backed class release entered a pool freelist\n", stderr);
                return 14;
            }
        }
    }
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
#if defined(__linux__)
    void *sized_slot = seen_pool_alloc(16);
    if (!rejected_release(sized_slot, 8) ||
        !rejected_release(sized_slot, 88) ||
        !rejected_release((char *)sized_slot + 1, 16)) return 15;
    seen_pool_free(sized_slot, 16);
    if (seen_memory_used_bytes() != initial ||
        seen_memory_reserved_bytes() != 12 * slab_bytes ||
        seen_memory_allocation_failures() != failures) return 16;
#endif
    puts("PASS: pooled live usage, recyclable reserve, physical budget");
    return 0;
}
