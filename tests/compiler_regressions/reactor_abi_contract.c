#include "reactor_abi.h"

#include <stdint.h>
#include <stdio.h>
#include <time.h>
#include <unistd.h>

static int fail(const char *message) {
    fprintf(stderr, "FAIL: %s\n", message);
    return 1;
}

static int64_t identity(int64_t value) { return value + 1; }

static int64_t delayed(int64_t value) {
    struct timespec delay = {0, 20000000};
    nanosleep(&delay, NULL);
    return value;
}

static int wait_completion(uint64_t pool, uint64_t expected_token,
                           int64_t expected_result) {
    for (int attempt = 0; attempt < 200; ++attempt) {
        uint64_t token = 0;
        int64_t result = 0;
        int32_t status = seen_blocking_pool_completion(pool, &token, &result);
        if (status == SEEN_REACTOR_OK)
            return token == expected_token && result == expected_result ? 0 : 1;
        if (status != SEEN_REACTOR_NOT_FOUND) return 1;
        struct timespec delay = {0, 1000000};
        nanosleep(&delay, NULL);
    }
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

    uint64_t pool = 0;
    if (seen_blocking_pool_create(1, 3, &pool) != SEEN_REACTOR_OK || !pool ||
        seen_blocking_pool_live_workers() != 1)
        return fail("fixed blocking workers");
    if (seen_blocking_pool_submit(pool, 1,
        (int64_t)(intptr_t)delayed, 7) != SEEN_REACTOR_OK)
        return fail("submit delayed blocking task");
    if (seen_blocking_pool_submit(pool, 2,
        (int64_t)(intptr_t)identity, 9) != SEEN_REACTOR_OK)
        return fail("submit bounded queued task");
    if (seen_blocking_pool_cancel(pool, 2) != SEEN_REACTOR_OK)
        return fail("cancel queued blocking task");
    if (wait_completion(pool, 1, 7) != 0)
        return fail("blocking completion");
    if (seen_blocking_pool_close(&pool) != SEEN_REACTOR_OK || pool != 0 ||
        seen_blocking_pool_close(&pool) != SEEN_REACTOR_OK ||
        seen_blocking_pool_live_workers() != 0)
        return fail("blocking pool shutdown and join");

    if (seen_blocking_pool_create(1, 3, &pool) != SEEN_REACTOR_OK)
        return fail("create completion saturation pool");
    if (seen_blocking_pool_submit(pool, 10,
        (int64_t)(intptr_t)delayed, 10) != SEEN_REACTOR_OK)
        return fail("submit active saturation task");
    struct timespec dispatch_delay = {0, 50000000};
    nanosleep(&dispatch_delay, NULL);
    for (uint64_t submitted = 11; submitted <= 13; ++submitted)
        if (seen_blocking_pool_submit(pool, submitted,
            (int64_t)(intptr_t)identity, (int64_t)submitted) !=
                SEEN_REACTOR_OK)
            return fail("fill bounded blocking queue");
    if (seen_blocking_pool_submit(pool, 14,
        (int64_t)(intptr_t)identity, 14) != SEEN_REACTOR_LIMIT)
        return fail("reject work beyond total completion capacity");
    if (wait_completion(pool, 10, 10) != 0 ||
        wait_completion(pool, 11, 12) != 0 ||
        wait_completion(pool, 12, 13) != 0 ||
        wait_completion(pool, 13, 14) != 0)
        return fail("retain every accepted blocking completion");
    if (seen_blocking_pool_close(&pool) != SEEN_REACTOR_OK ||
        seen_blocking_pool_live_workers() != 0)
        return fail("completion saturation pool cleanup");

    puts("PASS: REACTOR-001A-H native ABI, epoll, wakeup, generations, fairness, and pool");
    return 0;
#endif
}
