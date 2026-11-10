#define _POSIX_C_SOURCE 200809L
#include "src/executor_abi.h"
#include "sync_abi.h"
#include <assert.h>
#include <errno.h>
#include <pthread.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>

static _Atomic int create_calls;
static int fail_create_at;

int __real_pthread_create(pthread_t *, const pthread_attr_t *,
                         void *(*)(void *), void *);
int __wrap_pthread_create(pthread_t *thread, const pthread_attr_t *attributes,
                         void *(*entry)(void *), void *argument) {
    int call = atomic_fetch_add(&create_calls, 1) + 1;
    if (fail_create_at && call == fail_create_at) return EAGAIN;
    return __real_pthread_create(thread, attributes, entry, argument);
}

typedef struct Probe {
    pthread_mutex_t mutex;
    pthread_cond_t changed;
    int published;
    uint64_t handle;
    int entered;
} Probe;

static int64_t probe_entry(int64_t argument) {
    Probe *probe = (Probe *)(uintptr_t)argument;
    assert(seen_executor_current_owner() == (uint64_t)argument);
    assert(seen_executor_current_task() == 0);
    assert(seen_executor_swap_task(7) == 0);
    assert(seen_executor_swap_task(11) == 7);
    assert(seen_executor_current_task() == 11);
    assert(seen_executor_swap_task(0) == 11);
    pthread_mutex_lock(&probe->mutex);
    while (!probe->published) pthread_cond_wait(&probe->changed, &probe->mutex);
    uint64_t own_handle = probe->handle;
    probe->entered++;
    pthread_mutex_unlock(&probe->mutex);
    assert(seen_executor_thread_join(&own_handle) == SEEN_SYNC_INVALID);
    assert(own_handle == probe->handle);
    return 0;
}

static void init_probe(Probe *probe) {
    *probe = (Probe){0};
    assert(pthread_mutex_init(&probe->mutex, NULL) == 0);
    assert(pthread_cond_init(&probe->changed, NULL) == 0);
}

static void publish(Probe *probe, uint64_t handle) {
    pthread_mutex_lock(&probe->mutex);
    probe->handle = handle;
    probe->published = 1;
    pthread_cond_broadcast(&probe->changed);
    pthread_mutex_unlock(&probe->mutex);
}

static void destroy_probe(Probe *probe) {
    assert(probe->entered == 1);
    assert(pthread_cond_destroy(&probe->changed) == 0);
    assert(pthread_mutex_destroy(&probe->mutex) == 0);
}

int main(void) {
    const int64_t entry = (int64_t)(uintptr_t)probe_entry;
    const uint64_t stack = UINT64_C(1048576);
    uint64_t handle = 0;
    assert(seen_executor_current_owner() == 0);
    assert(seen_executor_current_task() == 0);
    assert(seen_executor_live_threads() == 0);
    assert(seen_executor_thread_create(0, 1, stack, &handle) == SEEN_SYNC_INVALID);
    assert(seen_executor_thread_create(entry, 0, stack, &handle) == SEEN_SYNC_INVALID);
    assert(seen_executor_thread_create(entry, 1, stack, NULL) == SEEN_SYNC_INVALID);
    assert(seen_executor_thread_create(entry, 1, 0, &handle) == SEEN_SYNC_LIMIT);
    assert(seen_executor_thread_create(entry, 1, UINT64_MAX, &handle) == SEEN_SYNC_LIMIT);
    assert(handle == 0 && seen_executor_live_threads() == 0);

    Probe probes[4];
    uint64_t handles[4] = {0};
    for (int i = 0; i < 4; ++i) {
        init_probe(&probes[i]);
        assert(seen_executor_thread_create(entry, (int64_t)(uintptr_t)&probes[i],
                                           stack, &handles[i]) == SEEN_SYNC_OK);
    }
    /* The harness compiles the adapter with four slots. Sleeping callbacks
     * retain all slots, so this tests saturation without excessive threads. */
    uint64_t rejected = 0;
    assert(seen_executor_thread_create(entry, 1, stack, &rejected) == SEEN_SYNC_LIMIT);
    assert(rejected == 0 && seen_executor_live_threads() == 4);
    uint64_t stale = handles[0];
    for (int i = 0; i < 4; ++i) {
        publish(&probes[i], handles[i]);
        assert(seen_executor_thread_join(&handles[i]) == SEEN_SYNC_OK);
        assert(handles[i] == 0);
        assert(seen_executor_thread_join(&handles[i]) == SEEN_SYNC_OK);
        destroy_probe(&probes[i]);
    }
    assert(seen_executor_live_threads() == 0);
    assert(seen_executor_thread_join(&stale) == SEEN_SYNC_INVALID);

    /* A native creation failure after a successful prefix never loses the
     * earlier join owner or publishes a handle for the failed thread. */
    atomic_store(&create_calls, 0);
    fail_create_at = 2;
    init_probe(&probes[0]);
    assert(seen_executor_thread_create(entry, (int64_t)(uintptr_t)&probes[0],
                                       stack, &handles[0]) == SEEN_SYNC_OK);
    assert(handles[0] != stale);
    assert(seen_executor_thread_join(&stale) == SEEN_SYNC_INVALID);
    assert(seen_executor_thread_create(entry, 1, stack, &handle) == SEEN_SYNC_LIMIT);
    assert(handle == 0 && seen_executor_live_threads() == 1);
    publish(&probes[0], handles[0]);
    assert(seen_executor_thread_join(&handles[0]) == SEEN_SYNC_OK);
    destroy_probe(&probes[0]);
    assert(seen_executor_live_threads() == 0);
    assert(seen_executor_thread_join(NULL) == SEEN_SYNC_INVALID);
    puts("PASS: executor OS adapter lifetime, saturation, TLS, and partial failure");
    return 0;
}
