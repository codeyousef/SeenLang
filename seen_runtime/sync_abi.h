#ifndef SEEN_SYNC_ABI_H
#define SEEN_SYNC_ABI_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

enum {
    SEEN_SYNC_OK = 0,
    SEEN_SYNC_INVALID = 1,
    SEEN_SYNC_LIMIT = 2,
    SEEN_SYNC_UNAVAILABLE = 3,
    SEEN_SYNC_UNSUPPORTED = 4,
    SEEN_SYNC_INTERRUPTED = 5,
    SEEN_SYNC_BUSY = 6,
    SEEN_SYNC_TIMEOUT = 7,
    SEEN_SYNC_CLOSED = 8
};

enum {
    SEEN_SYNC_RELAXED = 0,
    SEEN_SYNC_ACQUIRE = 1,
    SEEN_SYNC_RELEASE = 2,
    SEEN_SYNC_ACQ_REL = 3,
    SEEN_SYNC_SEQ_CST = 4
};

int32_t seen_sync_atomic_create(int64_t initial, uint64_t *out_handle);
int32_t seen_sync_atomic_load(uint64_t handle, int32_t order,
                              int64_t *out_value);
int32_t seen_sync_atomic_store(uint64_t handle, int64_t value,
                               int32_t order);
int32_t seen_sync_atomic_exchange(uint64_t handle, int64_t value,
                                  int32_t order, int64_t *out_previous);
int32_t seen_sync_atomic_fetch_add(uint64_t handle, int64_t value,
                                   int32_t order, int64_t *out_previous);
int32_t seen_sync_atomic_fetch_sub(uint64_t handle, int64_t value,
                                   int32_t order, int64_t *out_previous);
int32_t seen_sync_atomic_compare_exchange(uint64_t handle,
    int64_t *expected, int64_t desired, int32_t success_order,
    int32_t failure_order, int32_t *out_exchanged);
int32_t seen_sync_atomic_destroy(uint64_t *handle);

int32_t seen_sync_mutex_create(uint64_t *out_handle);
int32_t seen_sync_mutex_lock_until(uint64_t handle,
                                   int64_t deadline_nanoseconds);
int32_t seen_sync_mutex_try_lock(uint64_t handle);
int32_t seen_sync_mutex_unlock(uint64_t handle);
int32_t seen_sync_mutex_destroy(uint64_t *handle);

int32_t seen_sync_condition_create(uint64_t *out_handle);
int32_t seen_sync_condition_wait_until(uint64_t condition, uint64_t mutex,
                                       int64_t deadline_nanoseconds);
int32_t seen_sync_condition_notify_one(uint64_t handle);
int32_t seen_sync_condition_notify_all(uint64_t handle);
int32_t seen_sync_condition_destroy(uint64_t *handle);

int32_t seen_sync_rwlock_create(uint64_t *out_handle);
int32_t seen_sync_rwlock_read_lock_until(uint64_t handle,
                                        int64_t deadline_nanoseconds);
int32_t seen_sync_rwlock_write_lock_until(uint64_t handle,
                                         int64_t deadline_nanoseconds);
int32_t seen_sync_rwlock_try_read(uint64_t handle);
int32_t seen_sync_rwlock_try_write(uint64_t handle);
int32_t seen_sync_rwlock_unlock(uint64_t handle);
int32_t seen_sync_rwlock_destroy(uint64_t *handle);

uint64_t seen_sync_current_thread_id(void);
uint64_t seen_sync_live_atomics(void);
uint64_t seen_sync_live_mutexes(void);
uint64_t seen_sync_live_conditions(void);
uint64_t seen_sync_live_rwlocks(void);

#ifdef __cplusplus
}
#endif

#endif
