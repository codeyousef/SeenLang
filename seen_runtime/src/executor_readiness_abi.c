#if defined(__linux__) && !defined(_GNU_SOURCE)
#define _GNU_SOURCE
#endif
#include "executor_readiness_abi.h"
#if defined(__linux__) && defined(__x86_64__)
#include <errno.h>
#include <limits.h>
#include <stdatomic.h>
#include <stddef.h>
#include <string.h>
#include <sys/epoll.h>
#include <sys/eventfd.h>
#include <unistd.h>

_Static_assert(sizeof(struct epoll_event) <= 16, "Seen epoll event storage bound");
static _Atomic uint64_t seen_async_readiness_handles;
static int seen_async_readiness_fd(int64_t value) { return value >= 0 && value <= INT_MAX; }
static int32_t seen_async_readiness_errno(int error) {
    if (error == EBADF || error == EINVAL || error == EEXIST) return 1;
    if (error == ENOMEM || error == ENOSPC || error == EMFILE || error == ENFILE) return 2;
    if (error == ENOSYS || error == EPERM) return 4;
    if (error == EINTR) return 5;
    return 3;
}
int32_t seen_async_readiness_create(int64_t *queue, int64_t *wake) {
    if (!queue || !wake || *queue != -1 || *wake != -1) return 1;
    int q = epoll_create1(EPOLL_CLOEXEC);
    if (q < 0) return seen_async_readiness_errno(errno);
    int w = eventfd(0, EFD_CLOEXEC | EFD_NONBLOCK);
    if (w < 0) { int error = errno; close(q); return seen_async_readiness_errno(error); }
    struct epoll_event event; memset(&event, 0, sizeof(event));
    event.events = EPOLLIN; event.data.u64 = 0;
    if (epoll_ctl(q, EPOLL_CTL_ADD, w, &event) != 0) {
        int error = errno; close(w); close(q); return seen_async_readiness_errno(error);
    }
    *queue = q; *wake = w;
    atomic_fetch_add_explicit(&seen_async_readiness_handles, 2, memory_order_relaxed);
    return 0;
}
int32_t seen_async_readiness_register(int64_t queue, int64_t descriptor, int32_t flags, uint64_t identity) {
    if (!seen_async_readiness_fd(queue) || !seen_async_readiness_fd(descriptor) ||
        !identity || flags < 1 || flags > 3) return 1;
    struct epoll_event event; memset(&event, 0, sizeof(event));
    event.events = EPOLLERR | EPOLLHUP;
    if (flags & 1) event.events |= EPOLLIN;
    if (flags & 2) event.events |= EPOLLOUT;
    event.data.u64 = identity;
    return epoll_ctl((int)queue, EPOLL_CTL_ADD, (int)descriptor, &event) == 0 ?
        0 : seen_async_readiness_errno(errno);
}
int32_t seen_async_readiness_deregister(int64_t queue, int64_t descriptor) {
    if (!seen_async_readiness_fd(queue) || !seen_async_readiness_fd(descriptor)) return 1;
    if (epoll_ctl((int)queue, EPOLL_CTL_DEL, (int)descriptor, NULL) == 0 || errno == ENOENT) return 0;
    return seen_async_readiness_errno(errno);
}
int32_t seen_async_readiness_wakeup(int64_t wake) {
    if (!seen_async_readiness_fd(wake)) return 1;
    uint64_t value = 1;
    ssize_t written = write((int)wake, &value, sizeof(value));
    if (written == sizeof(value) || (written < 0 && errno == EAGAIN)) return 0;
    return written < 0 ? seen_async_readiness_errno(errno) : 3;
}
int32_t seen_async_readiness_consume_wake(int64_t wake) {
    if (!seen_async_readiness_fd(wake)) return 1;
    uint64_t value = 0;
    ssize_t received = read((int)wake, &value, sizeof(value));
    if (received == sizeof(value) || (received < 0 && errno == EAGAIN)) return 0;
    return received < 0 ? seen_async_readiness_errno(errno) : 3;
}
int32_t seen_async_readiness_poll(int64_t queue, void *events, uint64_t event_bytes,
    uint64_t maximum, int64_t timeout_nanos, uint64_t *count) {
    if (!seen_async_readiness_fd(queue) || !events || !count || !maximum || maximum > INT_MAX ||
        maximum > event_bytes / sizeof(struct epoll_event) || timeout_nanos < 0) return 1;
    int64_t millis = timeout_nanos / 1000000 + (timeout_nanos % 1000000 != 0);
    if (millis > INT_MAX) millis = INT_MAX;
    *count = 0;
    int found = epoll_wait((int)queue, events, (int)maximum, (int)millis);
    if (found < 0) return seen_async_readiness_errno(errno);
    *count = (uint64_t)found; return 0;
}
int32_t seen_async_readiness_event(const void *events, uint64_t event_bytes,
    uint64_t index, uint64_t *identity, int32_t *flags) {
    if (!events || !identity || !flags || index >= event_bytes / sizeof(struct epoll_event)) return 1;
    const struct epoll_event *event = &((const struct epoll_event *)events)[index];
    *identity = event->data.u64; *flags = 0;
    if (!*identity) { *flags = 32; return 0; }
    if (event->events & EPOLLIN) *flags |= 1;
    if (event->events & EPOLLOUT) *flags |= 2;
    if (event->events & EPOLLERR) *flags |= 4;
    if (event->events & EPOLLHUP) *flags |= 8;
    return 0;
}
int32_t seen_async_readiness_close(int64_t *queue, int64_t *wake) {
    if (!queue || !wake || *queue < -1 || *wake < -1 || *queue > INT_MAX || *wake > INT_MAX) return 1;
    // Linux consumes descriptor ownership before reporting delayed close
    // errors. Retain error information, never a reusable descriptor number.
    if (*wake >= 0) {
        int result = close((int)*wake), error = errno;
        *wake = -1; atomic_fetch_sub_explicit(&seen_async_readiness_handles, 1, memory_order_relaxed);
        if (result != 0 && error != EINTR) return seen_async_readiness_errno(error);
    }
    if (*queue >= 0) {
        int result = close((int)*queue), error = errno;
        *queue = -1; atomic_fetch_sub_explicit(&seen_async_readiness_handles, 1, memory_order_relaxed);
        if (result != 0 && error != EINTR) return seen_async_readiness_errno(error);
    }
    return 0;
}
uint64_t seen_async_readiness_live_handles(void) {
    return atomic_load_explicit(&seen_async_readiness_handles, memory_order_relaxed);
}
#else
int32_t seen_async_readiness_create(int64_t*q,int64_t*w) {(void)q;(void)w;return 4;}
int32_t seen_async_readiness_register(int64_t q,int64_t d,int32_t f,uint64_t i) {(void)q;(void)d;(void)f;(void)i;return 4;}
int32_t seen_async_readiness_deregister(int64_t q,int64_t d) {(void)q;(void)d;return 4;}
int32_t seen_async_readiness_wakeup(int64_t w) {(void)w;return 4;}
int32_t seen_async_readiness_consume_wake(int64_t w) {(void)w;return 4;}
int32_t seen_async_readiness_poll(int64_t q,void*e,uint64_t b,uint64_t m,int64_t t,uint64_t*c) {(void)q;(void)e;(void)b;(void)m;(void)t;(void)c;return 4;}
int32_t seen_async_readiness_event(const void*e,uint64_t b,uint64_t i,uint64_t*t,int32_t*f) {(void)e;(void)b;(void)i;(void)t;(void)f;return 4;}
int32_t seen_async_readiness_close(int64_t*q,int64_t*w) {(void)q;(void)w;return 4;}
uint64_t seen_async_readiness_live_handles(void) {return 0;}
#endif
