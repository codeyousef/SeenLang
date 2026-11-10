#ifndef SEEN_IOCP_ABI_H
#define SEEN_IOCP_ABI_H
#include <stdint.h>
/* Raw Win32 results: zero succeeds, otherwise a Win32 error code. On other
 * targets operations return ERROR_NOT_SUPPORTED (50), never an errno. */
uint32_t seen_iocp_create(uint32_t concurrency, int64_t *port);
uint32_t seen_iocp_associate(int64_t port, int64_t file, uint64_t key);
int64_t seen_iocp_utf16(const uint8_t *input, uint32_t bytes, uint16_t *output, uint32_t capacity);
uint32_t seen_iocp_open(const uint16_t *path, uint32_t access, uint32_t share,
    uint32_t creation, uint32_t flags, int64_t *file);
uint32_t seen_iocp_transfer(int64_t file, void *buffer, uint32_t bytes,
    void *overlapped, uint64_t overlapped_bytes, uint32_t write);
uint32_t seen_iocp_cancel(int64_t file, void *overlapped);
uint32_t seen_iocp_dequeue(int64_t port, uint32_t *bytes, uint64_t *key, uint64_t *overlapped);
#endif
