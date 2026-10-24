#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include "src/executor_readiness_abi.h"
#include <assert.h>
#include <stdint.h>
#include <errno.h>
#include <fcntl.h>
#include "reactor_abi.h"
#include <sys/eventfd.h>
#include <unistd.h>

extern int __real_close(int fd);
static int fail_close;
int __wrap_close(int fd) {
    int result = __real_close(fd);
    if (fail_close && result == 0) { fail_close = 0; errno = EIO; return -1; }
    return result;
}
int main(void) {
    uint64_t baseline = seen_async_readiness_live_handles();
    int64_t queue = -1, wake = -1;
    assert(seen_async_readiness_create(&queue, &wake) == 0);
    assert(seen_async_readiness_live_handles() == baseline + 2);
    int fd = eventfd(0, EFD_CLOEXEC | EFD_NONBLOCK); assert(fd >= 0);
    const uint64_t identity = UINT64_C(4294967297);
    assert(seen_async_readiness_register(queue, fd, 1, identity) == 0);
    assert(seen_async_readiness_register(queue, fd, 1, identity) == 1);
    _Alignas(16) unsigned char events[32];
    uint64_t count = 0, observed = 0; int32_t flags = 0;
    assert(seen_async_readiness_poll(queue, events, sizeof(events), 3, 0, &count) == 1);
    assert(seen_async_readiness_poll(queue, events, sizeof(events), 2, 0, &count) == 0 && count == 0);
    uint64_t value = 1; assert(write(fd, &value, sizeof(value)) == sizeof(value));
    assert(seen_async_readiness_poll(queue, events, sizeof(events), 2, 100000000, &count) == 0 && count == 1);
    assert(seen_async_readiness_event(events, sizeof(events), 0, &observed, &flags) == 0);
    assert(observed == identity && (flags & 1));
    assert(seen_async_readiness_deregister(queue, fd) == 0);
    assert(seen_async_readiness_deregister(queue, fd) == 0);
    assert(seen_async_readiness_wakeup(wake) == 0);
    assert(seen_async_readiness_wakeup(wake) == 0);
    assert(seen_async_readiness_poll(queue, events, sizeof(events), 2, 100000000, &count) == 0 && count == 1);
    assert(seen_async_readiness_event(events, sizeof(events), 0, &observed, &flags) == 0);
    assert(observed == 0 && flags == 32);
    assert(seen_async_readiness_consume_wake(wake) == 0);
    assert(seen_async_readiness_poll(queue, events, sizeof(events), 2, 0, &count) == 0 && count == 0);
    int old_wake = (int)wake;
    fail_close = 1;
    assert(seen_async_readiness_close(&queue, &wake) == 3 && wake == -1 && queue >= 0);
    assert(seen_async_readiness_live_handles() == baseline + 1);
    int replacement = eventfd(0, EFD_CLOEXEC | EFD_NONBLOCK); assert(replacement >= 0);
    assert(replacement == old_wake);
    assert(seen_async_readiness_close(&queue, &wake) == 0 && queue == -1 && wake == -1);
    assert(fcntl(replacement, F_GETFD) >= 0); /* retry did not close a reused fd */
    fail_close = 1;
    assert(seen_reactor_native_handle_close(replacement) == SEEN_REACTOR_CLOSED_ERROR);
    assert(seen_async_readiness_close(&queue, &wake) == 0);
    assert(seen_async_readiness_live_handles() == baseline);
    assert(close(fd) == 0);
    return 0;
}
