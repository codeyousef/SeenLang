#if defined(__linux__) && !defined(_GNU_SOURCE)
#define _GNU_SOURCE
#endif
#include "executor_completion_abi.h"
#include <stddef.h>
#include <stdlib.h>
#if defined(__linux__) && defined(__x86_64__)
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <linux/aio_abi.h>
#include <string.h>
#include <sys/eventfd.h>
#include <sys/syscall.h>
#include <time.h>
#include <unistd.h>

_Static_assert(sizeof(struct iocb) == 64, "Seen Linux AIO request layout");
static int32_t seen_async_completion_errno(int error) {
    if (error == EINVAL || error == EBADF || error == EFAULT) return 1;
    if (error == EAGAIN || error == ENOMEM || error == EMFILE || error == ENFILE) return 2;
    if (error == ENOSYS || error == EOPNOTSUPP) return 4;
    if (error == EINTR) return 5;
    return 3;
}
int32_t seen_async_completion_create(uint64_t capacity, uint64_t *context, int64_t *notification) {
    if (!context || !notification || *context != 0 || *notification != -1 ||
        capacity == 0 || capacity > 1048576) return 1;
    aio_context_t native = 0;
    if (syscall(SYS_io_setup, (unsigned)capacity, &native) != 0)
        return seen_async_completion_errno(errno);
    int fd = eventfd(0, EFD_CLOEXEC | EFD_NONBLOCK | EFD_SEMAPHORE);
    if (fd < 0) {
        int error = errno;
        if (syscall(SYS_io_destroy, native) != 0) abort();
        return seen_async_completion_errno(error);
    }
    *context = (uint64_t)native; *notification = fd; return 0;
}
int32_t seen_async_completion_submit(uint64_t context, int64_t notification,
    void *request, uint64_t request_bytes, int64_t descriptor, void *buffer,
    uint64_t bytes, int64_t offset, int32_t write, uint64_t token) {
    if (!context || notification < 0 || notification > INT_MAX || !request ||
        request_bytes != sizeof(struct iocb) || (uintptr_t)request % _Alignof(struct iocb) ||
        descriptor < 0 || descriptor > INT_MAX || !buffer || bytes == 0 ||
        bytes > SSIZE_MAX || offset < 0 || (write != 0 && write != 1) || !token) return 1;
    int flags = fcntl((int)descriptor, F_GETFL);
    if (flags < 0) return seen_async_completion_errno(errno);
    // Buffered Linux AIO can block during io_submit. Expose only explicit
    // direct-I/O descriptors; alignment and queue policy belong to Seen.
    if (!(flags & O_DIRECT)) return 4;
    struct iocb *value = request;
    memset(value, 0, sizeof(*value));
    value->aio_data = token; value->aio_lio_opcode = write ? IOCB_CMD_PWRITE : IOCB_CMD_PREAD;
    value->aio_fildes = (uint32_t)descriptor; value->aio_buf = (uint64_t)(uintptr_t)buffer;
    value->aio_nbytes = bytes; value->aio_offset = offset;
    value->aio_flags = IOCB_FLAG_RESFD; value->aio_resfd = (uint32_t)notification;
    struct iocb *requests[] = {value};
    long result = syscall(SYS_io_submit, (aio_context_t)context, 1L, requests);
    if (result == 1) return 0;
    return result < 0 ? seen_async_completion_errno(errno) : 2;
}
int32_t seen_async_completion_cancel(uint64_t context, void *request,
    uint64_t *token, int64_t *result) {
    if (!context || !request || !token || !result) return 1;
    struct io_event event;
    if (syscall(SYS_io_cancel, (aio_context_t)context, request, &event) != 0) {
        // EINVAL/EAGAIN commonly mean completion is already queued or the
        // driver cannot cancel. Seen retains request/buffer owners and drains.
        if (errno == EINVAL || errno == EAGAIN) return 8;
        return seen_async_completion_errno(errno);
    }
    *token = event.data; *result = event.res; return 0;
}
int32_t seen_async_completion_reap(uint64_t context, uint64_t *token, int64_t *result) {
    if (!context || !token || !result) return 1;
    struct io_event event; struct timespec timeout = {0, 0};
    long count = syscall(SYS_io_getevents, (aio_context_t)context, 0L, 1L, &event, &timeout);
    if (count < 0) return seen_async_completion_errno(errno);
    if (!count) return 8;
    *token = event.data; *result = event.res; return 0;
}
int32_t seen_async_completion_drain_notification(int64_t notification) {
    if (notification < 0 || notification > INT_MAX) return 1;
    uint64_t count = 0;
    ssize_t bytes = read((int)notification, &count, sizeof(count));
    if (bytes == sizeof(count) || (bytes < 0 && errno == EAGAIN)) return 0;
    return bytes < 0 ? seen_async_completion_errno(errno) : 3;
}
int32_t seen_async_completion_close(uint64_t *context) {
    if (!context) return 1;
    if (!*context) return 0;
    if (syscall(SYS_io_destroy, (aio_context_t)*context) != 0)
        return seen_async_completion_errno(errno);
    *context = 0; return 0;
}
#else
int32_t seen_async_completion_create(uint64_t n,uint64_t*c,int64_t*f) {(void)n;(void)c;(void)f;return 4;}
int32_t seen_async_completion_submit(uint64_t c,int64_t n,void*r,uint64_t z,int64_t f,void*b,uint64_t l,int64_t o,int32_t w,uint64_t t) {(void)c;(void)n;(void)r;(void)z;(void)f;(void)b;(void)l;(void)o;(void)w;(void)t;return 4;}
int32_t seen_async_completion_cancel(uint64_t c,void*r,uint64_t*t,int64_t*v) {(void)c;(void)r;(void)t;(void)v;return 4;}
int32_t seen_async_completion_reap(uint64_t c,uint64_t*t,int64_t*v) {(void)c;(void)t;(void)v;return 4;}
int32_t seen_async_completion_drain_notification(int64_t n) {(void)n;return 4;}
int32_t seen_async_completion_close(uint64_t*c) {(void)c;return 4;}
#endif
