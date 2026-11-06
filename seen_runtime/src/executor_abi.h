#ifndef SEEN_EXECUTOR_ABI_H
#define SEEN_EXECUTOR_ABI_H

#include <stdint.h>

/* Status values match sync_abi.h. The adapter owns only OS thread lifetime;
 * the Seen executor owns its queue, task states, cancellation and shutdown. */
typedef int64_t (*SeenExecutorEntry)(int64_t argument);

int32_t seen_executor_thread_create(int64_t entry, int64_t argument,
    uint64_t stack_bytes, uint64_t *out_handle);
int32_t seen_executor_thread_join(uint64_t *handle);
uint64_t seen_executor_current_owner(void);
int64_t seen_executor_current_task(void);
int64_t seen_executor_swap_task(int64_t task);
uint64_t seen_executor_live_threads(void);
int32_t seen_executor_validate_placement(int32_t cpu, int32_t node);
int32_t seen_executor_apply_placement(int32_t cpu, int32_t node);

#endif
