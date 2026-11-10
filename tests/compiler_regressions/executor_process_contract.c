#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include "src/executor_process_abi.h"
#include "src/executor_stream_abi.h"
#include <assert.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>
static void ready(int64_t fd) {
    struct pollfd item = {(int)fd, POLLIN, 0};
    assert(poll(&item, 1, 3000) == 1);
}
int main(void) {
    uint64_t baseline = seen_async_process_live_children();
    int64_t in_read = -1, in_write = -1, out_read = -1, out_write = -1;
    int64_t err_read = -1, err_write = -1, child = -1, pidfd = -1;
    assert(seen_async_process_pipe(1, &in_read, &in_write) == 0);
    assert(seen_async_process_pipe(0, &out_read, &out_write) == 0);
    assert(seen_async_process_pipe(0, &err_read, &err_write) == 0);
    assert(in_read >= 3 && in_write >= 3);
    assert(!(fcntl((int)in_read, F_GETFL) & O_NONBLOCK));
    assert(fcntl((int)in_write, F_GETFL) & O_NONBLOCK);
    const char *arguments[] = {"/bin/cat", NULL}, *environment[] = {NULL};
    assert(seen_async_process_spawn("/does/not/exist", arguments, environment,
        in_read, out_write, err_write, &child) == 1 && child == -1);
    assert(seen_async_process_spawn("/bin/cat", arguments, environment,
        in_read, out_write, err_write, &child) == 0 && child > 0);
    assert(seen_async_process_pidfd(child, &pidfd) == 0);
    assert(close((int)in_read) == 0); assert(close((int)out_write) == 0); assert(close((int)err_write) == 0);
    uint64_t count = 0;
    assert(seen_async_stream_write(in_write, "abc", 3, &count) == 0 && count == 3);
    assert(close((int)in_write) == 0);
    ready(out_read); char bytes[8] = {0};
    assert(seen_async_stream_read(out_read, bytes, sizeof(bytes), &count) == 0 && count == 3);
    assert(memcmp(bytes, "abc", 3) == 0);
    ready(pidfd); int32_t done = 0, code = -1, signal = -1;
    assert(seen_async_process_wait(child, 0, &done, &code, &signal) == 0 && done == 1 && code == 0 && signal == 0);
    assert(close((int)out_read) == 0); assert(close((int)err_read) == 0); assert(close((int)pidfd) == 0);
    // Child cancellation uses its pidfd, then drains the exact owned PID.
    in_read = -1; in_write = -1; out_read = -1; out_write = -1; err_read = -1; err_write = -1;
    child = -1; pidfd = -1;
    assert(seen_async_process_pipe(1, &in_read, &in_write) == 0);
    assert(seen_async_process_pipe(0, &out_read, &out_write) == 0);
    assert(seen_async_process_pipe(0, &err_read, &err_write) == 0);
    const char *sleep_arguments[] = {"/bin/sleep", "60", NULL};
    assert(seen_async_process_spawn("/bin/sleep", sleep_arguments, environment,
        in_read, out_write, err_write, &child) == 0);
    assert(seen_async_process_pidfd(child, &pidfd) == 0);
    assert(seen_async_process_signal(pidfd, SIGKILL) == 0);
    ready(pidfd);
    assert(seen_async_process_wait(child, 0, &done, &code, &signal) == 0 && done == 1 && signal == SIGKILL);
    assert(close((int)in_read) == 0); assert(close((int)in_write) == 0);
    assert(close((int)out_read) == 0); assert(close((int)out_write) == 0);
    assert(close((int)err_read) == 0); assert(close((int)err_write) == 0); assert(close((int)pidfd) == 0);
    assert(seen_async_process_live_children() == baseline);
    return 0;
}
