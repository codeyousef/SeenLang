#if !defined(_WIN32) && !defined(_POSIX_C_SOURCE)
#define _POSIX_C_SOURCE 200809L
#endif

#include "sync_abi.h"

#include <errno.h>
#include <limits.h>
#include <stdatomic.h>
#include <stdint.h>
#include <string.h>
#include <time.h>

#if defined(_WIN32)
#include <windows.h>
#else
#include <pthread.h>
#include <sched.h>
#include <unistd.h>
#endif

#define SEEN_SYNC_HANDLE_SLOTS 4096U
#define SEEN_SYNC_RW_READER_SLOTS 64U

enum SeenSyncHandleType {
    SEEN_SYNC_HANDLE_FREE = 0,
    SEEN_SYNC_HANDLE_ATOMIC = 1,
    SEEN_SYNC_HANDLE_MUTEX = 2,
    SEEN_SYNC_HANDLE_CONDITION = 3,
    SEEN_SYNC_HANDLE_RWLOCK = 4
};

typedef struct SeenSyncAtomic {
    _Atomic int64_t value;
} SeenSyncAtomic;

typedef struct SeenSyncMutex {
#if defined(_WIN32)
    CRITICAL_SECTION native;
#else
    pthread_mutex_t native;
#endif
    _Atomic uint32_t holders;
    _Atomic uint64_t owner;
} SeenSyncMutex;

typedef struct SeenSyncCondition {
#if defined(_WIN32)
    CONDITION_VARIABLE native;
#else
    pthread_cond_t native;
#endif
} SeenSyncCondition;

typedef struct SeenSyncRwLock {
#if defined(_WIN32)
    SRWLOCK native;
    _Atomic int32_t writer;
#else
    pthread_rwlock_t native;
#endif
    _Atomic uint32_t holders;
    _Atomic uint64_t writer_owner;
    uint64_t reader_owners[SEEN_SYNC_RW_READER_SLOTS];
    uint32_t reader_counts[SEEN_SYNC_RW_READER_SLOTS];
} SeenSyncRwLock;

typedef union SeenSyncStorage {
    SeenSyncAtomic atomic;
    SeenSyncMutex mutex;
    SeenSyncCondition condition;
    SeenSyncRwLock rwlock;
} SeenSyncStorage;

typedef struct SeenSyncHandleSlot {
    uint32_t generation;
    uint32_t references;
    int32_t type;
    int32_t closing;
    SeenSyncStorage storage;
} SeenSyncHandleSlot;

static SeenSyncHandleSlot seen_sync_slots[SEEN_SYNC_HANDLE_SLOTS];
static atomic_flag seen_sync_registry_guard = ATOMIC_FLAG_INIT;
static _Atomic uint64_t seen_sync_atomic_count = 0;
static _Atomic uint64_t seen_sync_mutex_count = 0;
static _Atomic uint64_t seen_sync_condition_count = 0;
static _Atomic uint64_t seen_sync_rwlock_count = 0;

static void seen_sync_registry_lock(void) {
    while (atomic_flag_test_and_set_explicit(&seen_sync_registry_guard,
                                              memory_order_acquire)) {
#if defined(_WIN32)
        (void)SwitchToThread();
#else
        (void)sched_yield();
#endif
    }
}

static void seen_sync_registry_unlock(void) {
    atomic_flag_clear_explicit(&seen_sync_registry_guard,
                               memory_order_release);
}

static uint64_t seen_sync_make_handle(uint32_t index, uint32_t generation) {
    return ((uint64_t)generation << 32) | ((uint64_t)index + UINT64_C(1));
}

static int seen_sync_decode_handle(uint64_t handle, uint32_t *out_index,
                                   uint32_t *out_generation) {
    uint32_t encoded_index = (uint32_t)(handle & UINT64_C(0xffffffff));
    uint32_t generation = (uint32_t)(handle >> 32);
    if (!out_index || !out_generation || encoded_index == 0 ||
        encoded_index > SEEN_SYNC_HANDLE_SLOTS || generation == 0)
        return 0;
    *out_index = encoded_index - 1U;
    *out_generation = generation;
    return 1;
}

static SeenSyncHandleSlot *seen_sync_reserve_slot(int32_t type,
                                                   uint64_t *out_handle,
                                                   uint32_t *out_index) {
    if (!out_handle || !out_index) return NULL;
    *out_handle = 0;
    seen_sync_registry_lock();
    for (uint32_t index = 0; index < SEEN_SYNC_HANDLE_SLOTS; ++index) {
        SeenSyncHandleSlot *slot = &seen_sync_slots[index];
        if (slot->type != SEEN_SYNC_HANDLE_FREE) continue;
        slot->generation += 1U;
        if (slot->generation == 0) slot->generation = 1U;
        slot->references = 0;
        slot->type = type;
        slot->closing = 1;
        memset(&slot->storage, 0, sizeof(slot->storage));
        *out_handle = seen_sync_make_handle(index, slot->generation);
        *out_index = index;
        return slot;
    }
    seen_sync_registry_unlock();
    return NULL;
}

static void seen_sync_publish_slot(uint32_t index) {
    seen_sync_slots[index].closing = 0;
    seen_sync_registry_unlock();
}

#if !defined(_WIN32)
static void seen_sync_abandon_slot(uint32_t index, uint64_t *out_handle) {
    SeenSyncHandleSlot *slot = &seen_sync_slots[index];
    slot->type = SEEN_SYNC_HANDLE_FREE;
    slot->closing = 0;
    slot->references = 0;
    if (out_handle) *out_handle = 0;
    seen_sync_registry_unlock();
}
#endif

static void *seen_sync_acquire(uint64_t handle, int32_t type,
                               uint32_t *out_index) {
    uint32_t index = 0;
    uint32_t generation = 0;
    if (!out_index ||
        !seen_sync_decode_handle(handle, &index, &generation))
        return NULL;
    seen_sync_registry_lock();
    SeenSyncHandleSlot *slot = &seen_sync_slots[index];
    if (slot->type != type || slot->generation != generation ||
        slot->closing || slot->references == UINT32_MAX) {
        seen_sync_registry_unlock();
        return NULL;
    }
    slot->references += 1U;
    *out_index = index;
    void *result = NULL;
    if (type == SEEN_SYNC_HANDLE_ATOMIC) result = &slot->storage.atomic;
    if (type == SEEN_SYNC_HANDLE_MUTEX) result = &slot->storage.mutex;
    if (type == SEEN_SYNC_HANDLE_CONDITION)
        result = &slot->storage.condition;
    if (type == SEEN_SYNC_HANDLE_RWLOCK) result = &slot->storage.rwlock;
    seen_sync_registry_unlock();
    return result;
}

static void seen_sync_release(uint32_t index) {
    seen_sync_registry_lock();
    if (index < SEEN_SYNC_HANDLE_SLOTS &&
        seen_sync_slots[index].references > 0)
        seen_sync_slots[index].references -= 1U;
    seen_sync_registry_unlock();
}

static int32_t seen_sync_begin_destroy(uint64_t handle, int32_t type,
                                       uint32_t *out_index,
                                       void **out_storage) {
    uint32_t index = 0;
    uint32_t generation = 0;
    if (!out_index || !out_storage ||
        !seen_sync_decode_handle(handle, &index, &generation))
        return SEEN_SYNC_INVALID;
    seen_sync_registry_lock();
    SeenSyncHandleSlot *slot = &seen_sync_slots[index];
    if (slot->type != type || slot->generation != generation ||
        slot->closing) {
        seen_sync_registry_unlock();
        return SEEN_SYNC_INVALID;
    }
    if (type == SEEN_SYNC_HANDLE_MUTEX &&
        atomic_load_explicit(&slot->storage.mutex.holders,
                             memory_order_acquire) != 0) {
        seen_sync_registry_unlock();
        return SEEN_SYNC_BUSY;
    }
    if (type == SEEN_SYNC_HANDLE_RWLOCK &&
        atomic_load_explicit(&slot->storage.rwlock.holders,
                             memory_order_acquire) != 0) {
        seen_sync_registry_unlock();
        return SEEN_SYNC_BUSY;
    }
    if (slot->references != 0) {
        seen_sync_registry_unlock();
        return SEEN_SYNC_BUSY;
    }
    slot->closing = 1;
    *out_index = index;
    *out_storage = &slot->storage;
    seen_sync_registry_unlock();
    return SEEN_SYNC_OK;
}

#if !defined(_WIN32)
static void seen_sync_cancel_destroy(uint32_t index) {
    seen_sync_registry_lock();
    if (index < SEEN_SYNC_HANDLE_SLOTS)
        seen_sync_slots[index].closing = 0;
    seen_sync_registry_unlock();
}
#endif

static void seen_sync_finish_destroy(uint32_t index, uint64_t *handle) {
    seen_sync_registry_lock();
    SeenSyncHandleSlot *slot = &seen_sync_slots[index];
    memset(&slot->storage, 0, sizeof(slot->storage));
    slot->references = 0;
    slot->type = SEEN_SYNC_HANDLE_FREE;
    slot->closing = 0;
    *handle = 0;
    seen_sync_registry_unlock();
}

static int seen_sync_valid_order(int32_t order) {
    return order >= SEEN_SYNC_RELAXED && order <= SEEN_SYNC_SEQ_CST;
}

static int seen_sync_valid_failure_order(int32_t success_order,
                                         int32_t failure_order) {
    if (!seen_sync_valid_order(success_order) ||
        !seen_sync_valid_order(failure_order) ||
        failure_order == SEEN_SYNC_RELEASE ||
        failure_order == SEEN_SYNC_ACQ_REL)
        return 0;
    if (failure_order == SEEN_SYNC_SEQ_CST)
        return success_order == SEEN_SYNC_SEQ_CST;
    if (failure_order == SEEN_SYNC_ACQUIRE)
        return success_order == SEEN_SYNC_ACQUIRE ||
            success_order == SEEN_SYNC_ACQ_REL ||
            success_order == SEEN_SYNC_SEQ_CST;
    return 1;
}

static memory_order seen_sync_order(int32_t order) {
    if (order == SEEN_SYNC_ACQUIRE) return memory_order_acquire;
    if (order == SEEN_SYNC_RELEASE) return memory_order_release;
    if (order == SEEN_SYNC_ACQ_REL) return memory_order_acq_rel;
    if (order == SEEN_SYNC_SEQ_CST) return memory_order_seq_cst;
    return memory_order_relaxed;
}

static int64_t seen_sync_monotonic_nanoseconds(void) {
#if defined(_WIN32)
    LARGE_INTEGER frequency;
    LARGE_INTEGER counter;
    if (!QueryPerformanceFrequency(&frequency) || frequency.QuadPart <= 0 ||
        !QueryPerformanceCounter(&counter)) return -1;
    return (int64_t)((counter.QuadPart / frequency.QuadPart) *
        INT64_C(1000000000) +
        ((counter.QuadPart % frequency.QuadPart) * INT64_C(1000000000)) /
        frequency.QuadPart);
#else
    struct timespec value;
    if (clock_gettime(CLOCK_MONOTONIC, &value) != 0) return -1;
    if (value.tv_sec > INT64_MAX / INT64_C(1000000000)) return INT64_MAX;
    return (int64_t)value.tv_sec * INT64_C(1000000000) + value.tv_nsec;
#endif
}

static int32_t seen_sync_deadline_status(int64_t deadline_nanoseconds) {
    if (deadline_nanoseconds == INT64_MAX) return SEEN_SYNC_OK;
    if (deadline_nanoseconds < 0) return SEEN_SYNC_TIMEOUT;
    int64_t now = seen_sync_monotonic_nanoseconds();
    if (now < 0) return SEEN_SYNC_UNAVAILABLE;
    return now >= deadline_nanoseconds ? SEEN_SYNC_TIMEOUT : SEEN_SYNC_OK;
}

static uint64_t seen_sync_native_thread_id(void) {
#if defined(_WIN32)
    return (uint64_t)GetCurrentThreadId();
#else
    pthread_t value = pthread_self();
    uint64_t result = 0;
    size_t count = sizeof(value) < sizeof(result) ? sizeof(value) :
        sizeof(result);
    memcpy(&result, &value, count);
    return result;
#endif
}

static int seen_sync_rw_reader_add(SeenSyncRwLock *value, uint64_t owner) {
    int free_index = -1;
    int added = 0;
    seen_sync_registry_lock();
    for (uint32_t i = 0; i < SEEN_SYNC_RW_READER_SLOTS; ++i) {
        if (value->reader_owners[i] == owner &&
            value->reader_counts[i] < UINT32_MAX) {
            value->reader_counts[i] += 1U;
            added = 1;
            break;
        }
        if (free_index < 0 && value->reader_counts[i] == 0)
            free_index = (int)i;
    }
    if (!added && free_index >= 0) {
        value->reader_owners[free_index] = owner;
        value->reader_counts[free_index] = 1U;
        added = 1;
    }
    seen_sync_registry_unlock();
    return added;
}

static int seen_sync_rw_reader_remove(SeenSyncRwLock *value,
                                      uint64_t owner) {
    int removed = 0;
    seen_sync_registry_lock();
    for (uint32_t i = 0; i < SEEN_SYNC_RW_READER_SLOTS; ++i) {
        if (value->reader_owners[i] == owner &&
            value->reader_counts[i] > 0) {
            value->reader_counts[i] -= 1U;
            if (value->reader_counts[i] == 0) value->reader_owners[i] = 0;
            removed = 1;
            break;
        }
    }
    seen_sync_registry_unlock();
    return removed;
}

static int32_t seen_sync_wait_turn(int64_t deadline_nanoseconds) {
    int32_t deadline_status = seen_sync_deadline_status(deadline_nanoseconds);
    if (deadline_status != SEEN_SYNC_OK) return deadline_status;
#if defined(_WIN32)
    Sleep(1);
#else
    struct timespec delay = {0, 1000000};
    while (nanosleep(&delay, &delay) != 0) {
        if (errno != EINTR) return SEEN_SYNC_UNAVAILABLE;
    }
#endif
    return SEEN_SYNC_OK;
}

int32_t seen_sync_atomic_create(int64_t initial, uint64_t *out_handle) {
    uint32_t index = 0;
    SeenSyncHandleSlot *slot = seen_sync_reserve_slot(
        SEEN_SYNC_HANDLE_ATOMIC, out_handle, &index);
    if (!slot) return out_handle ? SEEN_SYNC_LIMIT : SEEN_SYNC_INVALID;
    atomic_init(&slot->storage.atomic.value, initial);
    seen_sync_publish_slot(index);
    atomic_fetch_add_explicit(&seen_sync_atomic_count, 1,
                              memory_order_relaxed);
    return SEEN_SYNC_OK;
}

int32_t seen_sync_atomic_load(uint64_t handle, int32_t order,
                              int64_t *out_value) {
    if (!out_value || !seen_sync_valid_order(order) ||
        order == SEEN_SYNC_RELEASE || order == SEEN_SYNC_ACQ_REL)
        return SEEN_SYNC_INVALID;
    uint32_t index = 0;
    SeenSyncAtomic *value = (SeenSyncAtomic *)seen_sync_acquire(handle,
        SEEN_SYNC_HANDLE_ATOMIC, &index);
    if (!value) return SEEN_SYNC_INVALID;
    *out_value = atomic_load_explicit(&value->value, seen_sync_order(order));
    seen_sync_release(index);
    return SEEN_SYNC_OK;
}

int32_t seen_sync_atomic_store(uint64_t handle, int64_t next, int32_t order) {
    if (!seen_sync_valid_order(order) || order == SEEN_SYNC_ACQUIRE ||
        order == SEEN_SYNC_ACQ_REL) return SEEN_SYNC_INVALID;
    uint32_t index = 0;
    SeenSyncAtomic *value = (SeenSyncAtomic *)seen_sync_acquire(handle,
        SEEN_SYNC_HANDLE_ATOMIC, &index);
    if (!value) return SEEN_SYNC_INVALID;
    atomic_store_explicit(&value->value, next, seen_sync_order(order));
    seen_sync_release(index);
    return SEEN_SYNC_OK;
}

int32_t seen_sync_atomic_exchange(uint64_t handle, int64_t next,
                                  int32_t order, int64_t *out_previous) {
    if (!out_previous || !seen_sync_valid_order(order))
        return SEEN_SYNC_INVALID;
    uint32_t index = 0;
    SeenSyncAtomic *value = (SeenSyncAtomic *)seen_sync_acquire(handle,
        SEEN_SYNC_HANDLE_ATOMIC, &index);
    if (!value) return SEEN_SYNC_INVALID;
    *out_previous = atomic_exchange_explicit(&value->value, next,
                                              seen_sync_order(order));
    seen_sync_release(index);
    return SEEN_SYNC_OK;
}

int32_t seen_sync_atomic_fetch_add(uint64_t handle, int64_t increment,
                                   int32_t order, int64_t *out_previous) {
    if (!out_previous || !seen_sync_valid_order(order))
        return SEEN_SYNC_INVALID;
    uint32_t index = 0;
    SeenSyncAtomic *value = (SeenSyncAtomic *)seen_sync_acquire(handle,
        SEEN_SYNC_HANDLE_ATOMIC, &index);
    if (!value) return SEEN_SYNC_INVALID;
    *out_previous = atomic_fetch_add_explicit(&value->value, increment,
                                               seen_sync_order(order));
    seen_sync_release(index);
    return SEEN_SYNC_OK;
}

int32_t seen_sync_atomic_fetch_sub(uint64_t handle, int64_t decrement,
                                   int32_t order, int64_t *out_previous) {
    if (!out_previous || !seen_sync_valid_order(order))
        return SEEN_SYNC_INVALID;
    uint32_t index = 0;
    SeenSyncAtomic *value = (SeenSyncAtomic *)seen_sync_acquire(handle,
        SEEN_SYNC_HANDLE_ATOMIC, &index);
    if (!value) return SEEN_SYNC_INVALID;
    *out_previous = atomic_fetch_sub_explicit(&value->value, decrement,
                                               seen_sync_order(order));
    seen_sync_release(index);
    return SEEN_SYNC_OK;
}

int32_t seen_sync_atomic_compare_exchange(uint64_t handle,
    int64_t *expected, int64_t desired, int32_t success_order,
    int32_t failure_order, int32_t *out_exchanged) {
    if (!expected || !out_exchanged ||
        !seen_sync_valid_failure_order(success_order, failure_order))
        return SEEN_SYNC_INVALID;
    uint32_t index = 0;
    SeenSyncAtomic *value = (SeenSyncAtomic *)seen_sync_acquire(handle,
        SEEN_SYNC_HANDLE_ATOMIC, &index);
    if (!value) return SEEN_SYNC_INVALID;
    *out_exchanged = atomic_compare_exchange_strong_explicit(&value->value,
        expected, desired, seen_sync_order(success_order),
        seen_sync_order(failure_order)) ? 1 : 0;
    seen_sync_release(index);
    return SEEN_SYNC_OK;
}

int32_t seen_sync_atomic_destroy(uint64_t *handle) {
    if (!handle) return SEEN_SYNC_INVALID;
    if (*handle == 0) return SEEN_SYNC_OK;
    uint32_t index = 0;
    void *storage = NULL;
    int32_t status = seen_sync_begin_destroy(*handle,
        SEEN_SYNC_HANDLE_ATOMIC, &index, &storage);
    if (status != SEEN_SYNC_OK) return status;
    (void)storage;
    seen_sync_finish_destroy(index, handle);
    atomic_fetch_sub_explicit(&seen_sync_atomic_count, 1,
                              memory_order_relaxed);
    return SEEN_SYNC_OK;
}

int32_t seen_sync_mutex_create(uint64_t *out_handle) {
    uint32_t index = 0;
    SeenSyncHandleSlot *slot = seen_sync_reserve_slot(
        SEEN_SYNC_HANDLE_MUTEX, out_handle, &index);
    if (!slot) return out_handle ? SEEN_SYNC_LIMIT : SEEN_SYNC_INVALID;
#if defined(_WIN32)
    InitializeCriticalSection(&slot->storage.mutex.native);
#else
    if (pthread_mutex_init(&slot->storage.mutex.native, NULL) != 0) {
        seen_sync_abandon_slot(index, out_handle);
        return SEEN_SYNC_UNAVAILABLE;
    }
#endif
    atomic_init(&slot->storage.mutex.holders, 0);
    atomic_init(&slot->storage.mutex.owner, 0);
    seen_sync_publish_slot(index);
    atomic_fetch_add_explicit(&seen_sync_mutex_count, 1, memory_order_relaxed);
    return SEEN_SYNC_OK;
}

int32_t seen_sync_mutex_try_lock(uint64_t handle) {
    uint32_t index = 0;
    SeenSyncMutex *value = (SeenSyncMutex *)seen_sync_acquire(handle,
        SEEN_SYNC_HANDLE_MUTEX, &index);
    if (!value) return SEEN_SYNC_INVALID;
#if defined(_WIN32)
    int32_t status = TryEnterCriticalSection(&value->native) ? SEEN_SYNC_OK :
        SEEN_SYNC_BUSY;
#else
    int result = pthread_mutex_trylock(&value->native);
    int32_t status = result == 0 ? SEEN_SYNC_OK :
        result == EBUSY ? SEEN_SYNC_BUSY : SEEN_SYNC_UNAVAILABLE;
#endif
    if (status == SEEN_SYNC_OK) {
        uint32_t previous = atomic_fetch_add_explicit(&value->holders, 1,
            memory_order_acq_rel);
        if (previous == 0)
            atomic_store_explicit(&value->owner,
                seen_sync_native_thread_id(), memory_order_release);
    }
    seen_sync_release(index);
    return status;
}

int32_t seen_sync_mutex_lock_until(uint64_t handle,
                                   int64_t deadline_nanoseconds) {
    if (deadline_nanoseconds == INT64_MAX) {
        uint32_t index = 0;
        SeenSyncMutex *value = (SeenSyncMutex *)seen_sync_acquire(handle,
            SEEN_SYNC_HANDLE_MUTEX, &index);
        if (!value) return SEEN_SYNC_INVALID;
#if defined(_WIN32)
        EnterCriticalSection(&value->native);
        int32_t status = SEEN_SYNC_OK;
#else
        int result = pthread_mutex_lock(&value->native);
        int32_t status = result == 0 ? SEEN_SYNC_OK :
            result == EINTR ? SEEN_SYNC_INTERRUPTED : SEEN_SYNC_UNAVAILABLE;
#endif
        if (status == SEEN_SYNC_OK) {
            uint32_t previous = atomic_fetch_add_explicit(&value->holders, 1,
                memory_order_acq_rel);
            if (previous == 0)
                atomic_store_explicit(&value->owner,
                    seen_sync_native_thread_id(), memory_order_release);
        }
        seen_sync_release(index);
        return status;
    }
    for (;;) {
        int32_t status = seen_sync_deadline_status(deadline_nanoseconds);
        if (status != SEEN_SYNC_OK) return status;
        status = seen_sync_mutex_try_lock(handle);
        if (status != SEEN_SYNC_BUSY) return status;
        status = seen_sync_wait_turn(deadline_nanoseconds);
        if (status != SEEN_SYNC_OK) return status;
    }
}

int32_t seen_sync_mutex_unlock(uint64_t handle) {
    uint32_t index = 0;
    SeenSyncMutex *value = (SeenSyncMutex *)seen_sync_acquire(handle,
        SEEN_SYNC_HANDLE_MUTEX, &index);
    if (!value) return SEEN_SYNC_INVALID;
    uint32_t holders = atomic_load_explicit(&value->holders,
                                            memory_order_acquire);
    uint64_t owner = atomic_load_explicit(&value->owner,
                                          memory_order_acquire);
    if (holders == 0 || owner != seen_sync_native_thread_id()) {
        seen_sync_release(index);
        return SEEN_SYNC_INVALID;
    }
    uint32_t previous = atomic_fetch_sub_explicit(&value->holders, 1,
        memory_order_acq_rel);
    if (previous == 1)
        atomic_store_explicit(&value->owner, 0, memory_order_release);
#if defined(_WIN32)
    LeaveCriticalSection(&value->native);
    int32_t status = SEEN_SYNC_OK;
#else
    int32_t status = pthread_mutex_unlock(&value->native) == 0 ?
        SEEN_SYNC_OK : SEEN_SYNC_INVALID;
    if (status != SEEN_SYNC_OK) {
        atomic_store_explicit(&value->owner, owner, memory_order_release);
        atomic_fetch_add_explicit(&value->holders, 1, memory_order_acq_rel);
    }
#endif
    seen_sync_release(index);
    return status;
}

int32_t seen_sync_mutex_destroy(uint64_t *handle) {
    if (!handle) return SEEN_SYNC_INVALID;
    if (*handle == 0) return SEEN_SYNC_OK;
    uint32_t index = 0;
    void *storage = NULL;
    int32_t status = seen_sync_begin_destroy(*handle,
        SEEN_SYNC_HANDLE_MUTEX, &index, &storage);
    if (status != SEEN_SYNC_OK) return status;
    SeenSyncMutex *value = &((SeenSyncStorage *)storage)->mutex;
#if defined(_WIN32)
    DeleteCriticalSection(&value->native);
#else
    int result = pthread_mutex_destroy(&value->native);
    if (result != 0) {
        seen_sync_cancel_destroy(index);
        return result == EBUSY ? SEEN_SYNC_BUSY : SEEN_SYNC_INVALID;
    }
#endif
    seen_sync_finish_destroy(index, handle);
    atomic_fetch_sub_explicit(&seen_sync_mutex_count, 1,
                              memory_order_relaxed);
    return SEEN_SYNC_OK;
}

int32_t seen_sync_condition_create(uint64_t *out_handle) {
    uint32_t index = 0;
    SeenSyncHandleSlot *slot = seen_sync_reserve_slot(
        SEEN_SYNC_HANDLE_CONDITION, out_handle, &index);
    if (!slot) return out_handle ? SEEN_SYNC_LIMIT : SEEN_SYNC_INVALID;
#if defined(_WIN32)
    InitializeConditionVariable(&slot->storage.condition.native);
#else
    pthread_condattr_t attributes;
    if (pthread_condattr_init(&attributes) != 0) {
        seen_sync_abandon_slot(index, out_handle);
        return SEEN_SYNC_UNAVAILABLE;
    }
#if defined(CLOCK_MONOTONIC)
    (void)pthread_condattr_setclock(&attributes, CLOCK_MONOTONIC);
#endif
    int result = pthread_cond_init(&slot->storage.condition.native,
                                   &attributes);
    pthread_condattr_destroy(&attributes);
    if (result != 0) {
        seen_sync_abandon_slot(index, out_handle);
        return SEEN_SYNC_UNAVAILABLE;
    }
#endif
    seen_sync_publish_slot(index);
    atomic_fetch_add_explicit(&seen_sync_condition_count, 1,
                              memory_order_relaxed);
    return SEEN_SYNC_OK;
}

int32_t seen_sync_condition_wait_until(uint64_t condition_handle,
    uint64_t mutex_handle, int64_t deadline_nanoseconds) {
    uint32_t condition_index = 0;
    uint32_t mutex_index = 0;
    SeenSyncCondition *condition = (SeenSyncCondition *)seen_sync_acquire(
        condition_handle, SEEN_SYNC_HANDLE_CONDITION, &condition_index);
    if (!condition) return SEEN_SYNC_INVALID;
    SeenSyncMutex *mutex = (SeenSyncMutex *)seen_sync_acquire(mutex_handle,
        SEEN_SYNC_HANDLE_MUTEX, &mutex_index);
    if (!mutex) {
        seen_sync_release(condition_index);
        return SEEN_SYNC_INVALID;
    }
    if (atomic_load_explicit(&mutex->holders, memory_order_acquire) != 1 ||
        atomic_load_explicit(&mutex->owner, memory_order_acquire) !=
            seen_sync_native_thread_id()) {
        seen_sync_release(mutex_index);
        seen_sync_release(condition_index);
        return SEEN_SYNC_INVALID;
    }
    atomic_store_explicit(&mutex->owner, 0, memory_order_release);
    atomic_store_explicit(&mutex->holders, 0, memory_order_release);
#if defined(_WIN32)
    DWORD milliseconds = INFINITE;
    int32_t status = SEEN_SYNC_OK;
    if (deadline_nanoseconds != INT64_MAX) {
        int64_t now = seen_sync_monotonic_nanoseconds();
        if (now < 0) status = SEEN_SYNC_UNAVAILABLE;
        else if (now >= deadline_nanoseconds) status = SEEN_SYNC_TIMEOUT;
        else {
            int64_t remaining = deadline_nanoseconds - now;
            int64_t rounded = remaining / INT64_C(1000000);
            if (remaining % INT64_C(1000000) != 0) rounded += 1;
            if (rounded < 1) rounded = 1;
            if (rounded >= (int64_t)INFINITE) rounded = INFINITE - 1;
            milliseconds = (DWORD)rounded;
        }
    }
    if (status == SEEN_SYNC_OK &&
        !SleepConditionVariableCS(&condition->native, &mutex->native,
                                  milliseconds))
        status = GetLastError() == ERROR_TIMEOUT ? SEEN_SYNC_TIMEOUT :
            SEEN_SYNC_UNAVAILABLE;
#else
    int result;
    if (deadline_nanoseconds == INT64_MAX) {
        result = pthread_cond_wait(&condition->native, &mutex->native);
    } else if (deadline_nanoseconds < 0) {
        result = ETIMEDOUT;
    } else {
        struct timespec deadline = {
            (time_t)(deadline_nanoseconds / INT64_C(1000000000)),
            (long)(deadline_nanoseconds % INT64_C(1000000000))
        };
        result = pthread_cond_timedwait(&condition->native, &mutex->native,
                                        &deadline);
    }
    int32_t status = result == 0 ? SEEN_SYNC_OK :
        result == ETIMEDOUT ? SEEN_SYNC_TIMEOUT :
        result == EINTR ? SEEN_SYNC_INTERRUPTED : SEEN_SYNC_UNAVAILABLE;
#endif
    atomic_store_explicit(&mutex->owner, seen_sync_native_thread_id(),
                          memory_order_release);
    atomic_store_explicit(&mutex->holders, 1, memory_order_release);
    seen_sync_release(mutex_index);
    seen_sync_release(condition_index);
    return status;
}

int32_t seen_sync_condition_notify_one(uint64_t handle) {
    uint32_t index = 0;
    SeenSyncCondition *value = (SeenSyncCondition *)seen_sync_acquire(handle,
        SEEN_SYNC_HANDLE_CONDITION, &index);
    if (!value) return SEEN_SYNC_INVALID;
#if defined(_WIN32)
    WakeConditionVariable(&value->native);
    int32_t status = SEEN_SYNC_OK;
#else
    int32_t status = pthread_cond_signal(&value->native) == 0 ?
        SEEN_SYNC_OK : SEEN_SYNC_UNAVAILABLE;
#endif
    seen_sync_release(index);
    return status;
}

int32_t seen_sync_condition_notify_all(uint64_t handle) {
    uint32_t index = 0;
    SeenSyncCondition *value = (SeenSyncCondition *)seen_sync_acquire(handle,
        SEEN_SYNC_HANDLE_CONDITION, &index);
    if (!value) return SEEN_SYNC_INVALID;
#if defined(_WIN32)
    WakeAllConditionVariable(&value->native);
    int32_t status = SEEN_SYNC_OK;
#else
    int32_t status = pthread_cond_broadcast(&value->native) == 0 ?
        SEEN_SYNC_OK : SEEN_SYNC_UNAVAILABLE;
#endif
    seen_sync_release(index);
    return status;
}

int32_t seen_sync_condition_destroy(uint64_t *handle) {
    if (!handle) return SEEN_SYNC_INVALID;
    if (*handle == 0) return SEEN_SYNC_OK;
    uint32_t index = 0;
    void *storage = NULL;
    int32_t status = seen_sync_begin_destroy(*handle,
        SEEN_SYNC_HANDLE_CONDITION, &index, &storage);
    if (status != SEEN_SYNC_OK) return status;
    SeenSyncCondition *value = &((SeenSyncStorage *)storage)->condition;
#if !defined(_WIN32)
    int result = pthread_cond_destroy(&value->native);
    if (result != 0) {
        seen_sync_cancel_destroy(index);
        return result == EBUSY ? SEEN_SYNC_BUSY : SEEN_SYNC_INVALID;
    }
#else
    (void)value;
#endif
    seen_sync_finish_destroy(index, handle);
    atomic_fetch_sub_explicit(&seen_sync_condition_count, 1,
                              memory_order_relaxed);
    return SEEN_SYNC_OK;
}

int32_t seen_sync_rwlock_create(uint64_t *out_handle) {
    uint32_t index = 0;
    SeenSyncHandleSlot *slot = seen_sync_reserve_slot(
        SEEN_SYNC_HANDLE_RWLOCK, out_handle, &index);
    if (!slot) return out_handle ? SEEN_SYNC_LIMIT : SEEN_SYNC_INVALID;
#if defined(_WIN32)
    InitializeSRWLock(&slot->storage.rwlock.native);
    atomic_init(&slot->storage.rwlock.writer, 0);
#else
    if (pthread_rwlock_init(&slot->storage.rwlock.native, NULL) != 0) {
        seen_sync_abandon_slot(index, out_handle);
        return SEEN_SYNC_UNAVAILABLE;
    }
#endif
    atomic_init(&slot->storage.rwlock.holders, 0);
    atomic_init(&slot->storage.rwlock.writer_owner, 0);
    seen_sync_publish_slot(index);
    atomic_fetch_add_explicit(&seen_sync_rwlock_count, 1,
                              memory_order_relaxed);
    return SEEN_SYNC_OK;
}

int32_t seen_sync_rwlock_try_read(uint64_t handle) {
    uint32_t index = 0;
    SeenSyncRwLock *value = (SeenSyncRwLock *)seen_sync_acquire(handle,
        SEEN_SYNC_HANDLE_RWLOCK, &index);
    if (!value) return SEEN_SYNC_INVALID;
#if defined(_WIN32)
    int32_t status = TryAcquireSRWLockShared(&value->native) ? SEEN_SYNC_OK :
        SEEN_SYNC_BUSY;
#else
    int result = pthread_rwlock_tryrdlock(&value->native);
    int32_t status = result == 0 ? SEEN_SYNC_OK :
        result == EBUSY ? SEEN_SYNC_BUSY : SEEN_SYNC_UNAVAILABLE;
#endif
    if (status == SEEN_SYNC_OK && !seen_sync_rw_reader_add(value,
            seen_sync_native_thread_id())) {
#if defined(_WIN32)
        ReleaseSRWLockShared(&value->native);
#else
        (void)pthread_rwlock_unlock(&value->native);
#endif
        status = SEEN_SYNC_LIMIT;
    }
    if (status == SEEN_SYNC_OK)
        atomic_fetch_add_explicit(&value->holders, 1, memory_order_acq_rel);
    seen_sync_release(index);
    return status;
}

int32_t seen_sync_rwlock_try_write(uint64_t handle) {
    uint32_t index = 0;
    SeenSyncRwLock *value = (SeenSyncRwLock *)seen_sync_acquire(handle,
        SEEN_SYNC_HANDLE_RWLOCK, &index);
    if (!value) return SEEN_SYNC_INVALID;
#if defined(_WIN32)
    int32_t status = SEEN_SYNC_BUSY;
    if (TryAcquireSRWLockExclusive(&value->native)) {
        atomic_store_explicit(&value->writer, 1, memory_order_relaxed);
        atomic_store_explicit(&value->writer_owner,
            seen_sync_native_thread_id(), memory_order_release);
        status = SEEN_SYNC_OK;
    }
#else
    int result = pthread_rwlock_trywrlock(&value->native);
    int32_t status = result == 0 ? SEEN_SYNC_OK :
        result == EBUSY ? SEEN_SYNC_BUSY : SEEN_SYNC_UNAVAILABLE;
#endif
    if (status == SEEN_SYNC_OK) {
#if !defined(_WIN32)
        atomic_store_explicit(&value->writer_owner,
            seen_sync_native_thread_id(), memory_order_release);
#endif
        atomic_fetch_add_explicit(&value->holders, 1, memory_order_acq_rel);
    }
    seen_sync_release(index);
    return status;
}

static int32_t seen_sync_rwlock_until(uint64_t handle, int64_t deadline,
                                      int write) {
    for (;;) {
        int32_t status = seen_sync_deadline_status(deadline);
        if (status != SEEN_SYNC_OK) return status;
        status = write ? seen_sync_rwlock_try_write(handle) :
            seen_sync_rwlock_try_read(handle);
        if (status != SEEN_SYNC_BUSY) return status;
        status = seen_sync_wait_turn(deadline);
        if (status != SEEN_SYNC_OK) return status;
    }
}

int32_t seen_sync_rwlock_read_lock_until(uint64_t handle, int64_t deadline) {
    return seen_sync_rwlock_until(handle, deadline, 0);
}

int32_t seen_sync_rwlock_write_lock_until(uint64_t handle, int64_t deadline) {
    return seen_sync_rwlock_until(handle, deadline, 1);
}

int32_t seen_sync_rwlock_unlock(uint64_t handle) {
    uint32_t index = 0;
    SeenSyncRwLock *value = (SeenSyncRwLock *)seen_sync_acquire(handle,
        SEEN_SYNC_HANDLE_RWLOCK, &index);
    if (!value) return SEEN_SYNC_INVALID;
    if (atomic_load_explicit(&value->holders, memory_order_acquire) == 0) {
        seen_sync_release(index);
        return SEEN_SYNC_INVALID;
    }
    int32_t writer = atomic_load_explicit(&value->writer_owner,
                                          memory_order_acquire) != 0;
#if defined(_WIN32)
    if (writer && atomic_load_explicit(&value->writer_owner,
            memory_order_acquire) != seen_sync_native_thread_id()) {
        seen_sync_release(index);
        return SEEN_SYNC_INVALID;
    }
    if (!writer && !seen_sync_rw_reader_remove(value,
            seen_sync_native_thread_id())) {
        seen_sync_release(index);
        return SEEN_SYNC_INVALID;
    }
    atomic_fetch_sub_explicit(&value->holders, 1, memory_order_acq_rel);
    if (writer) {
        atomic_store_explicit(&value->writer, 0, memory_order_release);
        atomic_store_explicit(&value->writer_owner, 0,
                              memory_order_release);
        ReleaseSRWLockExclusive(&value->native);
    } else {
        ReleaseSRWLockShared(&value->native);
    }
    int32_t status = SEEN_SYNC_OK;
#else
    uint64_t writer_owner = atomic_load_explicit(&value->writer_owner,
                                                 memory_order_acquire);
    if (writer_owner != 0 &&
        atomic_load_explicit(&value->writer_owner, memory_order_acquire) !=
            seen_sync_native_thread_id()) {
        seen_sync_release(index);
        return SEEN_SYNC_INVALID;
    }
    if (!writer && !seen_sync_rw_reader_remove(value,
            seen_sync_native_thread_id())) {
        seen_sync_release(index);
        return SEEN_SYNC_INVALID;
    }
    atomic_fetch_sub_explicit(&value->holders, 1, memory_order_acq_rel);
    if (writer) atomic_store_explicit(&value->writer_owner, 0,
                                      memory_order_release);
    int32_t status = pthread_rwlock_unlock(&value->native) == 0 ?
        SEEN_SYNC_OK : SEEN_SYNC_INVALID;
    if (status != SEEN_SYNC_OK) {
        if (writer)
            atomic_store_explicit(&value->writer_owner, writer_owner,
                                  memory_order_release);
        else
            (void)seen_sync_rw_reader_add(value,
                seen_sync_native_thread_id());
        atomic_fetch_add_explicit(&value->holders, 1, memory_order_acq_rel);
    }
#endif
    seen_sync_release(index);
    return status;
}

int32_t seen_sync_rwlock_destroy(uint64_t *handle) {
    if (!handle) return SEEN_SYNC_INVALID;
    if (*handle == 0) return SEEN_SYNC_OK;
    uint32_t index = 0;
    void *storage = NULL;
    int32_t status = seen_sync_begin_destroy(*handle,
        SEEN_SYNC_HANDLE_RWLOCK, &index, &storage);
    if (status != SEEN_SYNC_OK) return status;
    SeenSyncRwLock *value = &((SeenSyncStorage *)storage)->rwlock;
#if !defined(_WIN32)
    int result = pthread_rwlock_destroy(&value->native);
    if (result != 0) {
        seen_sync_cancel_destroy(index);
        return result == EBUSY ? SEEN_SYNC_BUSY : SEEN_SYNC_INVALID;
    }
#else
    (void)value;
#endif
    seen_sync_finish_destroy(index, handle);
    atomic_fetch_sub_explicit(&seen_sync_rwlock_count, 1,
                              memory_order_relaxed);
    return SEEN_SYNC_OK;
}

uint64_t seen_sync_current_thread_id(void) {
    return seen_sync_native_thread_id();
}

uint64_t seen_sync_live_atomics(void) {
    return atomic_load_explicit(&seen_sync_atomic_count, memory_order_relaxed);
}

uint64_t seen_sync_live_mutexes(void) {
    return atomic_load_explicit(&seen_sync_mutex_count, memory_order_relaxed);
}

uint64_t seen_sync_live_conditions(void) {
    return atomic_load_explicit(&seen_sync_condition_count,
                                memory_order_relaxed);
}

uint64_t seen_sync_live_rwlocks(void) {
    return atomic_load_explicit(&seen_sync_rwlock_count, memory_order_relaxed);
}
