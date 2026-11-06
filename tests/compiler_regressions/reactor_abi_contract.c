#include "reactor_abi.h"

#include <stdint.h>
#include <stdio.h>
#include <time.h>
#include <unistd.h>

static int fail(const char *message) {
    fprintf(stderr, "FAIL: %s\n", message);
    return 1;
}

int main(void) {
#if !defined(__linux__)
    return fail("Linux epoll contract requires Linux");
#else
    if (seen_reactor_backend_capability(SEEN_REACTOR_BACKEND_EPOLL) !=
        SEEN_REACTOR_OK) return fail("epoll capability");
    if (seen_reactor_backend_capability(SEEN_REACTOR_BACKEND_KQUEUE) !=
        SEEN_REACTOR_UNSUPPORTED) return fail("kqueue capability diagnostic");
    if (seen_reactor_backend_capability(SEEN_REACTOR_BACKEND_IOCP) !=
        SEEN_REACTOR_UNSUPPORTED) return fail("IOCP capability diagnostic");

    uint64_t reactor = 0;
    int32_t backend = 0;
    if (seen_reactor_create(4, &reactor, &backend) != SEEN_REACTOR_OK ||
        !reactor || backend != SEEN_REACTOR_BACKEND_EPOLL)
        return fail("create bounded epoll reactor");
    if (seen_reactor_live_handles() != 1)
        return fail("reactor live-handle accounting");

    int descriptors[2];
    if (pipe(descriptors) != 0) return fail("create readiness pipe");
    if (seen_reactor_register(reactor, descriptors[0], SEEN_REACTOR_READABLE,
                              41, 1) != SEEN_REACTOR_OK)
        return fail("register readable handle");
    if (seen_reactor_deregister(reactor, descriptors[0], 41, 2) !=
        SEEN_REACTOR_NOT_FOUND) return fail("reject stale generation");
    const char byte = 'x';
    if (write(descriptors[1], &byte, 1) != 1) return fail("signal pipe");
    uint64_t count = 0;
    if (seen_reactor_poll(reactor, 100000000, 2, &count) != SEEN_REACTOR_OK ||
        count != 1) return fail("bounded epoll poll");
    uint64_t token = 0, generation = 0, progress = 0;
    int32_t flags = 0;
    int64_t native_status = 0;
    if (seen_reactor_event(reactor, 0, &token, &generation, &flags,
                           &native_status, &progress) != SEEN_REACTOR_OK ||
        token != 41 || generation != 1 ||
        !(flags & SEEN_REACTOR_READABLE)) return fail("readiness event mapping");
    if (seen_reactor_deregister(reactor, descriptors[0], 41, 1) !=
        SEEN_REACTOR_OK) return fail("generation-safe deregistration");
    close(descriptors[0]);
    close(descriptors[1]);

    if (seen_reactor_wakeup(reactor) != SEEN_REACTOR_OK ||
        seen_reactor_poll(reactor, 100000000, 1, &count) != SEEN_REACTOR_OK ||
        count != 1) return fail("cross-thread wakeup");
    if (seen_reactor_event(reactor, 0, &token, &generation, &flags,
                           &native_status, &progress) != SEEN_REACTOR_OK ||
        flags != SEEN_REACTOR_WAKEUP) return fail("wakeup event mapping");

    for (int sample = 0; sample < 30; ++sample) {
        if (seen_reactor_wakeup(reactor) != SEEN_REACTOR_OK ||
            seen_reactor_poll(reactor, 100000000, 1, &count) !=
                SEEN_REACTOR_OK || count != 1)
            return fail("30-sample wakeup performance corpus");
    }
    if (seen_reactor_close(&reactor) != SEEN_REACTOR_OK || reactor != 0 ||
        seen_reactor_close(&reactor) != SEEN_REACTOR_OK ||
        seen_reactor_live_handles() != 0)
        return fail("idempotent reactor cleanup");

    puts("PASS: REACTOR-001A-H native ABI, epoll, wakeup, generations and fairness");
    return 0;
#endif
}
