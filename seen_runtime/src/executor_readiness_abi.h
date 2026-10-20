#ifndef SEEN_EXECUTOR_READINESS_ABI_H
#define SEEN_EXECUTOR_READINESS_ABI_H
#include <stdint.h>
int32_t seen_async_readiness_create(int64_t *queue, int64_t *wake);
int32_t seen_async_readiness_register(int64_t queue, int64_t descriptor, int32_t flags, uint64_t identity);
int32_t seen_async_readiness_deregister(int64_t queue, int64_t descriptor);
int32_t seen_async_readiness_wakeup(int64_t wake);
int32_t seen_async_readiness_consume_wake(int64_t wake);
int32_t seen_async_readiness_poll(int64_t queue, void *events, uint64_t event_bytes,
    uint64_t maximum, int64_t timeout_nanos, uint64_t *count);
int32_t seen_async_readiness_event(const void *events, uint64_t event_bytes,
    uint64_t index, uint64_t *identity, int32_t *flags);
int32_t seen_async_readiness_close(int64_t *queue, int64_t *wake);
uint64_t seen_async_readiness_live_handles(void);
#endif
