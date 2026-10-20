#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include "src/executor_completion_abi.h"
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <linux/aio_abi.h>
#include <poll.h>
#include <stdarg.h>
#include <stdint.h>
#include <string.h>
#include <sys/syscall.h>
#include <unistd.h>

static struct iocb *pending;
static int cancel_pending = 1, have_completion = 0, destroy_fails = 0;
static int direct = 1;
int __wrap_fcntl(int fd, int operation, ...) {
    assert(fd == 123 && operation == F_GETFL);
    return direct ? O_DIRECT : 0;
}
long __wrap_syscall(long number, ...) {
    va_list args; va_start(args, number);
    long result = 0;
    if (number == SYS_io_setup) {
        unsigned capacity = va_arg(args, unsigned);
        aio_context_t *context = va_arg(args, aio_context_t *);
        assert(capacity == 2 && *context == 0); *context = 42;
    } else if (number == SYS_io_submit) {
        aio_context_t context = va_arg(args, aio_context_t);
        long count = va_arg(args, long); struct iocb **requests = va_arg(args, struct iocb **);
        assert(context == 42 && count == 1);
        pending = requests[0];
        assert(pending->aio_data == UINT64_C(4294967297));
        assert(pending->aio_nbytes == 512 && pending->aio_offset == 1024);
        assert(pending->aio_fildes == 123 && pending->aio_flags == IOCB_FLAG_RESFD);
        result = 1;
    } else if (number == SYS_io_cancel) {
        aio_context_t context = va_arg(args, aio_context_t);
        struct iocb *request = va_arg(args, struct iocb *);
        struct io_event *event = va_arg(args, struct io_event *);
        assert(context == 42 && request == pending);
        if (cancel_pending) { errno = EAGAIN; result = -1; }
        else { memset(event, 0, sizeof(*event)); event->data = pending->aio_data; event->res = -ECANCELED; }
    } else if (number == SYS_io_getevents) {
        aio_context_t context = va_arg(args, aio_context_t);
        long minimum = va_arg(args, long), maximum = va_arg(args, long);
        struct io_event *event = va_arg(args, struct io_event *);
        assert(context == 42 && minimum == 0 && maximum == 1);
        if (have_completion) {
            memset(event, 0, sizeof(*event)); event->data = pending->aio_data; event->res = 256;
            have_completion = 0; result = 1;
        }
    } else if (number == SYS_io_destroy) {
        assert(va_arg(args, aio_context_t) == 42);
        if (destroy_fails) { errno = EBUSY; result = -1; }
    } else { assert(0 && "unexpected completion syscall"); }
    va_end(args); return result;
}
int main(void) {
    uint64_t context = 0; int64_t notification = -1;
    assert(seen_async_completion_create(0, &context, &notification) == 1);
    assert(seen_async_completion_create(2, &context, &notification) == 0);
    assert(context == 42 && notification >= 0);
    _Alignas(64) unsigned char request[64]; _Alignas(512) unsigned char data[512];
    uint64_t token = UINT64_C(4294967297), returned = 0; int64_t result = 0;
    direct = 0;
    assert(seen_async_completion_submit(context, notification, request, 64, 123,
        data, 512, 1024, 0, token) == 4);
    direct = 1;
    assert(seen_async_completion_submit(context, notification, request, 63, 123,
        data, 512, 1024, 0, token) == 1);
    assert(seen_async_completion_submit(context, notification, request, 64, 123,
        data, 512, 1024, 0, token) == 0);
    assert(seen_async_completion_cancel(context, request, &returned, &result) == 8);
    assert(seen_async_completion_reap(context, &returned, &result) == 8);
    have_completion = 1;
    assert(seen_async_completion_reap(context, &returned, &result) == 0);
    assert(returned == token && result == 256);
    cancel_pending = 0;
    assert(seen_async_completion_cancel(context, request, &returned, &result) == 0);
    assert(returned == token && result == -ECANCELED);
    uint64_t count = 2;
    assert(write((int)notification, &count, sizeof(count)) == sizeof(count));
    assert(seen_async_completion_drain_notification(notification) == 0);
    struct pollfd ready = {(int)notification, POLLIN, 0};
    assert(poll(&ready, 1, 0) == 1);
    assert(seen_async_completion_drain_notification(notification) == 0);
    assert(poll(&ready, 1, 0) == 0);
    assert(seen_async_completion_drain_notification(notification) == 0);
    destroy_fails = 1;
    assert(seen_async_completion_close(&context) == 3 && context == 42);
    destroy_fails = 0;
    assert(seen_async_completion_close(&context) == 0 && context == 0);
    assert(seen_async_completion_close(&context) == 0);
    assert(close((int)notification) == 0);
    return 0;
}
