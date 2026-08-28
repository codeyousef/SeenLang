#define _POSIX_C_SOURCE 200809L

#include "sync_abi.h"

#include <pthread.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

enum { WORKERS = 4, ITERATIONS = 10000 };

static int64_t monotonic_ns(void) {
    struct timespec value;
    if (clock_gettime(CLOCK_MONOTONIC, &value) != 0) return -1;
    return (int64_t)value.tv_sec * INT64_C(1000000000) + value.tv_nsec;
}

static void require(int condition, const char *message) {
    if (!condition) {
        fprintf(stderr, "FAIL: %s\n", message);
        exit(1);
    }
}

typedef struct AtomicWorker {
    uint64_t handle;
} AtomicWorker;

static void *atomic_worker(void *opaque) {
    AtomicWorker *worker = (AtomicWorker *)opaque;
    for (int i = 0; i < ITERATIONS; ++i) {
        int64_t previous = 0;
        require(seen_sync_atomic_fetch_add(worker->handle, 1,
                    SEEN_SYNC_ACQ_REL, &previous) == SEEN_SYNC_OK,
                "atomic fetch-add");
    }
    return NULL;
}

typedef struct MutexWorker {
    uint64_t handle;
    int64_t *value;
} MutexWorker;

static void *mutex_worker(void *opaque) {
    MutexWorker *worker = (MutexWorker *)opaque;
    for (int i = 0; i < ITERATIONS; ++i) {
        require(seen_sync_mutex_lock_until(worker->handle, INT64_MAX) ==
                    SEEN_SYNC_OK,
                "mutex lock");
        *worker->value += 1;
        require(seen_sync_mutex_unlock(worker->handle) == SEEN_SYNC_OK,
                "mutex unlock");
    }
    return NULL;
}

typedef struct ConditionWorker {
    uint64_t mutex;
    uint64_t condition;
    _Atomic int started;
    int ready;
    int payload;
} ConditionWorker;

static void *condition_worker(void *opaque) {
    ConditionWorker *worker = (ConditionWorker *)opaque;
    require(seen_sync_mutex_lock_until(worker->mutex, INT64_MAX) ==
                SEEN_SYNC_OK,
            "condition mutex lock");
    atomic_store_explicit(&worker->started, 1, memory_order_release);
    while (!worker->ready) {
        require(seen_sync_condition_wait_until(worker->condition,
                    worker->mutex, INT64_MAX) == SEEN_SYNC_OK,
                "condition wait");
    }
    require(worker->payload == 73, "condition payload visibility");
    require(seen_sync_mutex_unlock(worker->mutex) == SEEN_SYNC_OK,
            "condition mutex unlock");
    return NULL;
}

typedef struct TimeoutWorker {
    uint64_t mutex;
    int32_t status;
} TimeoutWorker;

static void *timeout_worker(void *opaque) {
    TimeoutWorker *worker = (TimeoutWorker *)opaque;
    int64_t deadline = monotonic_ns() + INT64_C(20000000);
    worker->status = seen_sync_mutex_lock_until(worker->mutex, deadline);
    return NULL;
}

static void *wrong_unlock_worker(void *opaque) {
    TimeoutWorker *worker = (TimeoutWorker *)opaque;
    worker->status = seen_sync_mutex_unlock(worker->mutex);
    return NULL;
}

static void *wrong_rw_unlock_worker(void *opaque) {
    TimeoutWorker *worker = (TimeoutWorker *)opaque;
    worker->status = seen_sync_rwlock_unlock(worker->mutex);
    return NULL;
}

int main(void) {
    const uint64_t atomics = seen_sync_live_atomics();
    const uint64_t mutexes = seen_sync_live_mutexes();
    const uint64_t conditions = seen_sync_live_conditions();
    const uint64_t rwlocks = seen_sync_live_rwlocks();

    uint64_t atomic = 0;
    require(seen_sync_atomic_create(0, &atomic) == SEEN_SYNC_OK,
            "atomic create");
    pthread_t threads[WORKERS];
    AtomicWorker atomic_worker_data = {atomic};
    for (int i = 0; i < WORKERS; ++i)
        require(pthread_create(&threads[i], NULL, atomic_worker,
                    &atomic_worker_data) == 0,
                "atomic thread create");
    for (int i = 0; i < WORKERS; ++i)
        require(pthread_join(threads[i], NULL) == 0, "atomic thread join");
    int64_t atomic_value = 0;
    require(seen_sync_atomic_load(atomic, SEEN_SYNC_ACQUIRE,
                &atomic_value) == SEEN_SYNC_OK,
            "atomic load");
    require(atomic_value == (int64_t)WORKERS * ITERATIONS,
            "atomic result");
    int64_t expected = atomic_value;
    int32_t exchanged = 0;
    require(seen_sync_atomic_compare_exchange(atomic, &expected, 7,
                SEEN_SYNC_ACQ_REL, SEEN_SYNC_ACQUIRE, &exchanged) ==
                SEEN_SYNC_OK && exchanged == 1,
            "atomic compare exchange");
    expected = 7;
    require(seen_sync_atomic_compare_exchange(atomic, &expected, 8,
                SEEN_SYNC_RELAXED, SEEN_SYNC_ACQUIRE, &exchanged) ==
                SEEN_SYNC_INVALID,
            "compare-exchange failure order rejection");
    uint64_t stale_atomic = atomic;
    require(seen_sync_atomic_destroy(&atomic) == SEEN_SYNC_OK && atomic == 0,
            "atomic destroy");
    require(seen_sync_atomic_load(stale_atomic, SEEN_SYNC_RELAXED,
                &atomic_value) == SEEN_SYNC_INVALID,
            "stale atomic rejection");

    uint64_t mutex = 0;
    require(seen_sync_mutex_create(&mutex) == SEEN_SYNC_OK, "mutex create");
    require(seen_sync_mutex_lock_until(mutex, monotonic_ns() - 1) ==
                SEEN_SYNC_TIMEOUT,
            "expired mutex deadline rejects immediate acquisition");
    int64_t mutex_value = 0;
    MutexWorker mutex_worker_data = {mutex, &mutex_value};
    for (int i = 0; i < WORKERS; ++i)
        require(pthread_create(&threads[i], NULL, mutex_worker,
                    &mutex_worker_data) == 0,
                "mutex thread create");
    for (int i = 0; i < WORKERS; ++i)
        require(pthread_join(threads[i], NULL) == 0, "mutex thread join");
    require(mutex_value == (int64_t)WORKERS * ITERATIONS, "mutex result");
    require(seen_sync_atomic_load(mutex, SEEN_SYNC_RELAXED,
                &atomic_value) == SEEN_SYNC_INVALID,
            "typed handle rejection");

    require(seen_sync_mutex_lock_until(mutex, INT64_MAX) == SEEN_SYNC_OK,
            "timeout owner lock");
    uint64_t held_mutex = mutex;
    require(seen_sync_mutex_destroy(&held_mutex) == SEEN_SYNC_BUSY &&
                held_mutex == mutex,
            "held mutex destroy rejection");
    TimeoutWorker wrong_unlock_data = {mutex, SEEN_SYNC_OK};
    pthread_t wrong_unlock_thread;
    require(pthread_create(&wrong_unlock_thread, NULL, wrong_unlock_worker,
                &wrong_unlock_data) == 0,
            "wrong unlock thread create");
    require(pthread_join(wrong_unlock_thread, NULL) == 0,
            "wrong unlock thread join");
    require(wrong_unlock_data.status == SEEN_SYNC_INVALID,
            "wrong-thread mutex unlock rejection");
    TimeoutWorker timeout_worker_data = {mutex, SEEN_SYNC_INVALID};
    pthread_t timeout_thread;
    require(pthread_create(&timeout_thread, NULL, timeout_worker,
                &timeout_worker_data) == 0,
            "timeout thread create");
    require(pthread_join(timeout_thread, NULL) == 0, "timeout thread join");
    require(timeout_worker_data.status == SEEN_SYNC_TIMEOUT,
            "mutex timeout");
    require(seen_sync_mutex_unlock(mutex) == SEEN_SYNC_OK,
            "timeout owner unlock");
    require(seen_sync_mutex_destroy(&mutex) == SEEN_SYNC_OK,
            "mutex destroy");

    ConditionWorker condition_worker_data = {0, 0, 0, 0, 0};
    atomic_init(&condition_worker_data.started, 0);
    require(seen_sync_mutex_create(&condition_worker_data.mutex) ==
                SEEN_SYNC_OK,
            "condition mutex create");
    require(seen_sync_condition_create(&condition_worker_data.condition) ==
                SEEN_SYNC_OK,
            "condition create");
    pthread_t condition_thread;
    require(pthread_create(&condition_thread, NULL, condition_worker,
                &condition_worker_data) == 0,
            "condition thread create");
    while (!atomic_load_explicit(&condition_worker_data.started,
                                memory_order_acquire)) {
        struct timespec delay = {0, 1000000};
        (void)nanosleep(&delay, NULL);
    }
    require(seen_sync_mutex_lock_until(condition_worker_data.mutex,
                INT64_MAX) == SEEN_SYNC_OK,
            "condition producer lock");
    condition_worker_data.payload = 73;
    condition_worker_data.ready = 1;
    require(seen_sync_condition_notify_one(condition_worker_data.condition) ==
                SEEN_SYNC_OK,
            "condition notify");
    require(seen_sync_mutex_unlock(condition_worker_data.mutex) ==
                SEEN_SYNC_OK,
            "condition producer unlock");
    require(pthread_join(condition_thread, NULL) == 0,
            "condition thread join");
    require(seen_sync_condition_destroy(&condition_worker_data.condition) ==
                SEEN_SYNC_OK,
            "condition destroy");
    require(seen_sync_mutex_destroy(&condition_worker_data.mutex) ==
                SEEN_SYNC_OK,
            "condition mutex destroy");

    uint64_t rwlock = 0;
    require(seen_sync_rwlock_create(&rwlock) == SEEN_SYNC_OK,
            "rwlock create");
    require(seen_sync_rwlock_read_lock_until(rwlock, monotonic_ns() - 1) ==
                SEEN_SYNC_TIMEOUT,
            "expired rwlock read deadline rejects immediate acquisition");
    require(seen_sync_rwlock_write_lock_until(rwlock, monotonic_ns() - 1) ==
                SEEN_SYNC_TIMEOUT,
            "expired rwlock write deadline rejects immediate acquisition");
    require(seen_sync_rwlock_read_lock_until(rwlock, INT64_MAX) ==
                SEEN_SYNC_OK,
            "rwlock read");
    TimeoutWorker wrong_rw_unlock_data = {rwlock, SEEN_SYNC_OK};
    pthread_t wrong_rw_unlock_thread;
    require(pthread_create(&wrong_rw_unlock_thread, NULL,
                wrong_rw_unlock_worker, &wrong_rw_unlock_data) == 0,
            "wrong rw unlock thread create");
    require(pthread_join(wrong_rw_unlock_thread, NULL) == 0,
            "wrong rw unlock thread join");
    require(wrong_rw_unlock_data.status == SEEN_SYNC_INVALID,
            "wrong-thread rw reader unlock rejection");
    uint64_t held_rwlock = rwlock;
    require(seen_sync_rwlock_destroy(&held_rwlock) == SEEN_SYNC_BUSY &&
                held_rwlock == rwlock,
            "held rwlock destroy rejection");
    require(seen_sync_rwlock_unlock(rwlock) == SEEN_SYNC_OK,
            "rwlock read unlock");
    require(seen_sync_rwlock_write_lock_until(rwlock, INT64_MAX) ==
                SEEN_SYNC_OK,
            "rwlock write");
    require(seen_sync_rwlock_unlock(rwlock) == SEEN_SYNC_OK,
            "rwlock write unlock");
    require(seen_sync_rwlock_destroy(&rwlock) == SEEN_SYNC_OK,
            "rwlock destroy");

    uint64_t bounded_handles[4096] = {0};
    for (int i = 0; i < 4096; ++i)
        require(seen_sync_atomic_create(i, &bounded_handles[i]) ==
                    SEEN_SYNC_OK,
                "bounded registry fill");
    uint64_t overflow_handle = 0;
    require(seen_sync_atomic_create(0, &overflow_handle) == SEEN_SYNC_LIMIT &&
                overflow_handle == 0,
            "bounded registry limit");
    for (int i = 0; i < 4096; ++i)
        require(seen_sync_atomic_destroy(&bounded_handles[i]) ==
                    SEEN_SYNC_OK,
                "bounded registry cleanup");

    require(seen_sync_current_thread_id() != 0, "thread identity");
    require(seen_sync_live_atomics() == atomics, "atomic leak");
    require(seen_sync_live_mutexes() == mutexes, "mutex leak");
    require(seen_sync_live_conditions() == conditions, "condition leak");
    require(seen_sync_live_rwlocks() == rwlocks, "rwlock leak");
    puts("PASS: native synchronization ABI contract");
    return 0;
}
