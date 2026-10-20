#ifndef SEEN_IO_URING_ABI_H
#define SEEN_IO_URING_ABI_H
#include <stdint.h>
/* Linux results are the raw syscall result or negative Linux errno. Seen owns
 * bounds, ring layout, ownership, scheduling, classification and retry policy. */
int64_t seen_uring_setup(uint32_t entries, void *parameters, uint64_t bytes);
int64_t seen_uring_enter(int64_t ring, uint32_t submitted);
int64_t seen_uring_register(int64_t ring, uint32_t opcode, const void *argument, uint32_t count);
int64_t seen_uring_map(int64_t ring, uint64_t offset, uint64_t bytes, uint64_t *address);
int64_t seen_uring_unmap(void *address, uint64_t bytes);
int64_t seen_uring_eventfd_create(void);
int64_t seen_uring_eventfd_drain(int64_t descriptor);
int64_t seen_uring_page_size(void);
/* Defined beside SeenFsFile in seen_runtime.c; duplicates its OS descriptor. */
int64_t seen_uring_duplicate_file(uint64_t source);
uint32_t seen_uring_load_acquire(const uint32_t *address);
void seen_uring_store_release(uint32_t *address, uint32_t value);
#endif
