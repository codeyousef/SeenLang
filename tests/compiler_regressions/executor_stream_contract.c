#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include "src/executor_stream_abi.h"
#include <assert.h>
#include <fcntl.h>
#include <signal.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>
int main(void) {
    int pipe_fds[2]; assert(pipe2(pipe_fds, O_NONBLOCK | O_CLOEXEC) == 0);
    assert(seen_async_stream_validate(pipe_fds[0]) == 0);
    char buffer[8192]; memset(buffer, 'x', sizeof(buffer)); uint64_t completed = 99;
    assert(seen_async_stream_read(pipe_fds[0], buffer, sizeof(buffer), &completed) == 3 && completed == 0);
    assert(seen_async_stream_write(pipe_fds[1], "abc", 3, &completed) == 0 && completed == 3);
    assert(seen_async_stream_read(pipe_fds[0], buffer, sizeof(buffer), &completed) == 0 && completed == 3);
    assert(memcmp(buffer, "abc", 3) == 0);
    assert(fcntl(pipe_fds[1], F_SETPIPE_SZ, 4096) == 4096);
    assert(seen_async_stream_write(pipe_fds[1], buffer, sizeof(buffer), &completed) == 0);
    assert(completed > 0 && completed < sizeof(buffer));
    assert(seen_async_stream_write(pipe_fds[1], buffer, sizeof(buffer), &completed) == 3 && completed == 0);
    assert(close(pipe_fds[0]) == 0);
    assert(seen_async_stream_write(pipe_fds[1], buffer, 1, &completed) == 10 && completed == 0);
    sigset_t block, previous, pending; sigemptyset(&block); sigaddset(&block, SIGPIPE);
    assert(sigprocmask(SIG_BLOCK, &block, &previous) == 0);
    assert(raise(SIGPIPE) == 0);
    assert(seen_async_stream_write(pipe_fds[1], buffer, 1, &completed) == 10);
    assert(sigpending(&pending) == 0 && sigismember(&pending, SIGPIPE) == 1);
    int signal = 0; assert(sigwait(&block, &signal) == 0 && signal == SIGPIPE);
    assert(sigprocmask(SIG_SETMASK, &previous, NULL) == 0);
    assert(close(pipe_fds[1]) == 0);
    int regular = open("/dev/null", O_RDONLY | O_NONBLOCK | O_CLOEXEC); assert(regular >= 0);
    assert(seen_async_stream_validate(regular) == 4); assert(close(regular) == 0);
    return 0;
}
