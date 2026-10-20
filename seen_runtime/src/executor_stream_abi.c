#if defined(__linux__) && !defined(_GNU_SOURCE)
#define _GNU_SOURCE
#endif
#include "executor_stream_abi.h"
#if defined(__linux__) && defined(__x86_64__)
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <pthread.h>
#include <signal.h>
#include <stddef.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

static int32_t seen_async_stream_errno(int error) {
    if (error == EBADF || error == EINVAL || error == ENOTSOCK) return 1;
    if (error == EAGAIN || error == EWOULDBLOCK || error == EINTR) return 3;
    if (error == ENOMEM || error == ENOBUFS) return 2;
    if (error == ENOSYS || error == EOPNOTSUPP) return 4;
    if (error == EPIPE) return 10; /* closed peer: not retryable */
    return 11; /* other permanent OS failure */
}
int32_t seen_async_stream_validate(int64_t descriptor) {
    if (descriptor < 0 || descriptor > INT_MAX) return 1;
    int flags = fcntl((int)descriptor, F_GETFL);
    if (flags < 0) return seen_async_stream_errno(errno);
    struct stat info;
    if (fstat((int)descriptor, &info) != 0) return seen_async_stream_errno(errno);
    if (!(flags & O_NONBLOCK) || (!S_ISFIFO(info.st_mode) && !S_ISSOCK(info.st_mode))) return 4;
    return 0;
}
int32_t seen_async_stream_read(int64_t descriptor, void *buffer, uint64_t bytes, uint64_t *completed) {
    if (!buffer || !completed || !bytes || bytes > SSIZE_MAX) return 1;
    *completed = 0;
    int32_t status = seen_async_stream_validate(descriptor);
    if (status) return status;
    ssize_t count = read((int)descriptor, buffer, (size_t)bytes);
    if (count < 0) return seen_async_stream_errno(errno);
    *completed = (uint64_t)count; return 0;
}
int32_t seen_async_stream_write(int64_t descriptor, const void *buffer, uint64_t bytes, uint64_t *completed) {
    if (!buffer || !completed || !bytes || bytes > SSIZE_MAX) return 1;
    *completed = 0;
    int32_t status = seen_async_stream_validate(descriptor);
    if (status) return status;
    // Normalize per-thread SIGPIPE behavior without changing process signal
    // handlers or consuming a signal already pending before this OS call.
    sigset_t block, previous, pending;
    sigemptyset(&block); sigaddset(&block, SIGPIPE);
    int error = pthread_sigmask(SIG_BLOCK, &block, &previous);
    if (error) return seen_async_stream_errno(error);
    if (sigpending(&pending) != 0) {
        error = errno; pthread_sigmask(SIG_SETMASK, &previous, NULL);
        return seen_async_stream_errno(error);
    }
    int was_pending = sigismember(&pending, SIGPIPE);
    ssize_t count = write((int)descriptor, buffer, (size_t)bytes);
    error = count < 0 ? errno : 0;
    if (count < 0 && error == EPIPE && !was_pending) {
        struct timespec immediate = {0, 0};
        while (sigtimedwait(&block, NULL, &immediate) < 0 && errno == EINTR) {}
    }
    int restore = pthread_sigmask(SIG_SETMASK, &previous, NULL);
    if (count >= 0) *completed = (uint64_t)count;
    if (restore) return seen_async_stream_errno(restore);
    return error ? seen_async_stream_errno(error) : 0;
}
#else
int32_t seen_async_stream_validate(int64_t d) {(void)d;return 4;}
int32_t seen_async_stream_read(int64_t d,void*b,uint64_t n,uint64_t*c) {(void)d;(void)b;(void)n;(void)c;return 4;}
int32_t seen_async_stream_write(int64_t d,const void*b,uint64_t n,uint64_t*c) {(void)d;(void)b;(void)n;(void)c;return 4;}
#endif
