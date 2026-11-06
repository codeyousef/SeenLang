#ifndef SEEN_EXECUTOR_COMPLETION_ABI_H
#define SEEN_EXECUTOR_COMPLETION_ABI_H
#include <stdint.h>
int32_t seen_async_completion_create(uint64_t capacity, uint64_t *context, int64_t *notification);
int32_t seen_async_completion_submit(uint64_t context, int64_t notification,
    void *request, uint64_t request_bytes, int64_t descriptor, void *buffer,
    uint64_t bytes, int64_t offset, int32_t write, uint64_t token);
int32_t seen_async_completion_cancel(uint64_t context, void *request,
    uint64_t *token, int64_t *result);
int32_t seen_async_completion_reap(uint64_t context, uint64_t *token, int64_t *result);
int32_t seen_async_completion_drain_notification(int64_t notification);
int32_t seen_async_completion_close(uint64_t *context);
int32_t seen_async_completion_duplicate_file(uint64_t source, int64_t *descriptor);
#endif
