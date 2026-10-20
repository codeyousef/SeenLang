#define _POSIX_C_SOURCE 200809L
#include "seen_runtime.h"
#include <assert.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

void *seen_pool_alloc(int64_t);
void seen_pool_free(void *, int64_t);

static void *allocate(void *raw) {
    uintptr_t identity = (uintptr_t)raw;
    /* Each worker owns every pointer it mutates; only allocator state is
     * shared. Vary size classes and exercise budgeted reallocations too. */
    for (int round = 0; round < 10000; ++round) {
        unsigned char *values[10];
        for (int slot = 0; slot < 10; ++slot) {
            size_t size = (size_t)(slot + 1) * 8;
            values[slot] = seen_pool_alloc((int64_t)size);
            assert(values[slot]);
            memset(values[slot], (int)(identity + (uintptr_t)slot), size);
        }
        void *large = seen_try_malloc(1024);
        assert(large);
        large = seen_try_realloc(large, 1024, 2048);
        assert(large);
        seen_pool_free(large, 2048);
        for (int slot = 9; slot >= 0; --slot) {
            size_t size = (size_t)(slot + 1) * 8;
            for (size_t i = 0; i < size; ++i)
                assert(values[slot][i] == (unsigned char)(identity + (uintptr_t)slot));
            seen_pool_free(values[slot], (int64_t)size);
        }
        SeenMemorySnapshot snapshot = seen_memory_snapshot();
        assert(snapshot.usedBytes >= 0);
        assert(snapshot.remainingBytes >= 0);
        assert(snapshot.usedBytes <= snapshot.peakBytes);
    }
    return NULL;
}

int main(int argc, char **argv) {
    seen_runtime_init(argc, argv);
    seen_memory_set_limit_bytes(INT64_C(67108864));
    const int64_t baseline = seen_memory_used_bytes();
    pthread_t workers[4];
    for (uintptr_t i = 0; i < 4; ++i)
        assert(pthread_create(&workers[i], NULL, allocate, (void *)(i + 1)) == 0);
    /* Fork while allocators are active. The child must not inherit a lock
     * stranded in a vanished thread; the harness enforces a wall deadline. */
    pid_t child = fork();
    assert(child >= 0);
    if (child == 0) {
        void *value = seen_pool_alloc(24);
        assert(value);
        seen_pool_free(value, 24);
        _exit(0);
    }
    for (int i = 0; i < 4; ++i) assert(pthread_join(workers[i], NULL) == 0);
    int status = 0;
    assert(waitpid(child, &status, 0) == child);
    assert(WIFEXITED(status) && WEXITSTATUS(status) == 0);
    assert(seen_memory_used_bytes() == baseline);
    puts("PASS: concurrent allocator ownership, accounting, and fork cleanup");
    return 0;
}
