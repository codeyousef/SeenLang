#ifndef SEEN_EXECUTOR_PROCESS_ABI_H
#define SEEN_EXECUTOR_PROCESS_ABI_H
#include <stdint.h>
int32_t seen_async_process_pipe(int32_t parent_write, int64_t *read_fd, int64_t *write_fd);
int32_t seen_async_process_spawn(const void *program, const void *arguments, const void *environment,
    int64_t input, int64_t output, int64_t error, int64_t *pid);
int32_t seen_async_process_pidfd(int64_t pid, int64_t *descriptor);
int32_t seen_async_process_signal(int64_t descriptor, int32_t signal);
int32_t seen_async_process_kill_unreaped(int64_t pid);
int32_t seen_async_process_wait(int64_t pid, int32_t blocking, int32_t *done,
    int32_t *exit_code, int32_t *signal);
uint64_t seen_async_process_live_children(void);
#endif
