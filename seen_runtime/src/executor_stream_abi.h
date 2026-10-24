#ifndef SEEN_EXECUTOR_STREAM_ABI_H
#define SEEN_EXECUTOR_STREAM_ABI_H
#include <stdint.h>
int32_t seen_async_stream_validate(int64_t descriptor);
int32_t seen_async_stream_read(int64_t descriptor, void *buffer, uint64_t bytes, uint64_t *completed);
int32_t seen_async_stream_write(int64_t descriptor, const void *buffer, uint64_t bytes, uint64_t *completed);
#endif
