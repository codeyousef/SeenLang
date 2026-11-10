#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <time.h>
#include <unistd.h>

static int faults;
static int staging_syncs;
static int clock_expired;
static int replacement = -1;
static int failed_open_fd = -1;
static int failed_open_closes;

static int matches(int fd, const char *target) {
    char descriptor[64], path[PATH_MAX + 1];
    if (!target) return 0;
    int length = snprintf(descriptor, sizeof descriptor, "/proc/self/fd/%d", fd);
    if (length <= 0 || (size_t)length >= sizeof descriptor) return 0;
    ssize_t count = readlink(descriptor, path, sizeof path - 1);
    if (count < 0) return 0;
    path[count] = '\0';
    return strcmp(path, target) == 0;
}

int fstat(int fd, struct stat *value) {
    int status = (int)syscall(SYS_fstat, fd, value);
    const char *mode = getenv("SEEN_CACHE_FAULT_MODE");
    if (status == 0 && !faults && mode && strcmp(mode, "fstat-error") == 0 &&
        matches(fd, getenv("SEEN_CACHE_FAULT_STAGE"))) {
        failed_open_fd = fd;
        faults++; errno = EIO; return -1;
    }
    return status;
}

int clock_gettime(clockid_t clock, struct timespec *value) {
    int status = (int)syscall(SYS_clock_gettime, clock, value);
    if (status == 0 && clock == CLOCK_MONOTONIC && clock_expired) value->tv_sec += 60;
    return status;
}

int fsync(int fd) {
    const char *mode = getenv("SEEN_CACHE_FAULT_MODE");
    int stage = matches(fd, getenv("SEEN_CACHE_FAULT_STAGE"));
    int directory = matches(fd, getenv("SEEN_CACHE_FAULT_DIRECTORY"));
    int status = (int)syscall(SYS_fsync, fd);
    if (status != 0 || !mode) return status;
    if (stage) staging_syncs++;
    if (!faults && stage && strcmp(mode, "expire") == 0) {
        faults++; clock_expired = 1;
    } else if (!faults && ((stage && strcmp(mode, "sync-error") == 0) ||
                           (directory && strcmp(mode, "directory-error") == 0))) {
        faults++; errno = EIO; return -1;
    }
    return 0;
}

int close(int fd) {
    const char *mode = getenv("SEEN_CACHE_FAULT_MODE");
    int failed_open = failed_open_fd >= 0 && fd == failed_open_fd;
    if (failed_open || (!faults && mode && strcmp(mode, "close-error") == 0 &&
        matches(fd, getenv("SEEN_CACHE_FAULT_STAGE")))) {
        if (syscall(SYS_close, fd) != 0) _Exit(90);
        replacement = (int)syscall(SYS_openat, AT_FDCWD, "/dev/null", O_RDONLY | O_CLOEXEC, 0);
        if (replacement != fd) _Exit(91);
        if (failed_open) { failed_open_fd = -1; failed_open_closes++; }
        else faults++;
        errno = EINTR; return -1;
    }
    return (int)syscall(SYS_close, fd);
}

__attribute__((destructor)) static void verify_fault_witness(void) {
    const char *mode = getenv("SEEN_CACHE_FAULT_MODE");
    int post_create = mode && strcmp(mode, "fstat-error") == 0;
    if (faults != 1 || staging_syncs != (post_create ? 0 : 1) ||
        failed_open_fd != -1 || failed_open_closes != post_create) {
        fputs("FAIL: cache publication did not exercise the designated native fault\n", stderr);
        _Exit(92);
    }
    if (replacement >= 0) {
        if (fcntl(replacement, F_GETFD) < 0 || syscall(SYS_close, replacement) != 0) _Exit(93);
    }
    fputs("PASS: designated cache publication fault executed once\n", stderr);
}
