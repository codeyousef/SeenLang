#ifndef SEEN_REACTOR_ABI_H
#define SEEN_REACTOR_ABI_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

enum {
    SEEN_REACTOR_OK = 0,
    SEEN_REACTOR_INVALID = 1,
    SEEN_REACTOR_LIMIT = 2,
    SEEN_REACTOR_UNAVAILABLE = 3,
    SEEN_REACTOR_UNSUPPORTED = 4,
    SEEN_REACTOR_INTERRUPTED = 5,
    SEEN_REACTOR_NOT_FOUND = 6,
    SEEN_REACTOR_BUSY = 7
};

enum {
    SEEN_REACTOR_BACKEND_EPOLL = 1,
    SEEN_REACTOR_BACKEND_KQUEUE = 2,
    SEEN_REACTOR_BACKEND_IOCP = 3
};

enum {
    SEEN_REACTOR_READABLE = 1,
    SEEN_REACTOR_WRITABLE = 2,
    SEEN_REACTOR_ERROR = 4,
    SEEN_REACTOR_HANGUP = 8,
    SEEN_REACTOR_COMPLETION = 16,
    SEEN_REACTOR_WAKEUP = 32
};

int32_t seen_reactor_backend_capability(int32_t backend);
int32_t seen_reactor_create(uint64_t capacity, uint64_t *out_handle,
                            int32_t *out_backend);
int32_t seen_reactor_register(uint64_t handle, int64_t native_handle,
                              int32_t interest, uint64_t token,
                              uint64_t generation);
int32_t seen_reactor_deregister(uint64_t handle, int64_t native_handle,
                                uint64_t token, uint64_t generation);
int32_t seen_reactor_wakeup(uint64_t handle);
int32_t seen_reactor_poll(uint64_t handle, int64_t timeout_nanoseconds,
                          uint64_t max_events, uint64_t *out_count);
int32_t seen_reactor_event(uint64_t handle, uint64_t index,
                           uint64_t *out_token, uint64_t *out_generation,
                           int32_t *out_flags, int64_t *out_native_status,
                           uint64_t *out_progress);
int32_t seen_reactor_close(uint64_t *handle);
uint64_t seen_reactor_live_handles(void);
int32_t seen_reactor_native_handle_close(int64_t native_handle);

int32_t seen_blocking_pool_create(uint64_t workers, uint64_t queue_capacity,
                                  uint64_t *out_handle);
int32_t seen_blocking_pool_submit(uint64_t handle, uint64_t token,
                                  int64_t function_address, int64_t argument);
int32_t seen_blocking_pool_cancel(uint64_t handle, uint64_t token);
int32_t seen_blocking_pool_completion(uint64_t handle, uint64_t *out_token,
                                      int64_t *out_result);
int32_t seen_blocking_pool_close(uint64_t *handle);
uint64_t seen_blocking_pool_live_workers(void);

#ifdef __cplusplus
}
#endif

#endif
