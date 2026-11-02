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

/* Linux preload consumer, not an SDK substitute. Compile with:
 * cc -shared -fPIC -Wall -Wextra -Werror vulkan_cache_read_fault_driver.c -o DRIVER.so
 * Run FIXTURE ABSENT_ABSOLUTE_PATH MODE with LD_PRELOAD=DRIVER.so,
 * SEEN_CACHE_READ_TARGET=ABSENT_ABSOLUTE_PATH and SEEN_CACHE_READ_MODE=MODE.
 * The fixture creates its real envelope before opening it O_RDONLY|O_NONBLOCK.
 * No fault applies to the writer, another path, or another descriptor kind.
 */
static int metadata_calls;
static int read_calls;
static int close_calls;
static int injections;
static int expired;
static int selected_fd = -1;

static int mode_is(const char *expected) {
    const char *mode = getenv("SEEN_CACHE_READ_MODE");
    return mode && strcmp(mode, expected) == 0;
}

static int target_reader(int fd) {
    const char *target = getenv("SEEN_CACHE_READ_TARGET");
    char descriptor[64], path[PATH_MAX + 1];
    if (!target || target[0] != '/') return 0;
    int flags = fcntl(fd, F_GETFL);
    if (flags < 0 || (flags & O_ACCMODE) != O_RDONLY || !(flags & O_NONBLOCK)) return 0;
    int length = snprintf(descriptor, sizeof descriptor, "/proc/self/fd/%d", fd);
    if (length <= 0 || (size_t)length >= sizeof descriptor) return 0;
    ssize_t count = readlink(descriptor, path, sizeof path - 1);
    if (count < 0) return 0;
    path[count] = '\0';
    return strcmp(path, target) == 0;
}

int fstat(int fd, struct stat *value) {
    int status = (int)syscall(SYS_fstat, fd, value);
    int saved_errno = errno;
    if (status == 0 && target_reader(fd)) {
        if (selected_fd != -1 && selected_fd != fd) _Exit(90);
        selected_fd = fd;
        metadata_calls++;
        /* First fstat belongs to File.open; second is the loader's metadata
         * admission, after an opaque file owner has actually been acquired. */
        if (metadata_calls == 2 && mode_is("metadata-error")) {
            injections++; errno = EIO; return -1;
        }
    }
    errno = saved_errno;
    return status;
}

static void truncate_target(int fd) {
    const char *target = getenv("SEEN_CACHE_READ_TARGET");
    int writer = (int)syscall(SYS_openat, AT_FDCWD, target, O_WRONLY | O_NOFOLLOW | O_CLOEXEC, 0);
    if (writer < 0) _Exit(91);
    struct stat reader_stat, writer_stat;
    if (syscall(SYS_fstat, fd, &reader_stat) != 0 ||
        syscall(SYS_fstat, writer, &writer_stat) != 0 ||
        reader_stat.st_dev != writer_stat.st_dev || reader_stat.st_ino != writer_stat.st_ino ||
        syscall(SYS_ftruncate, writer, 0) != 0) _Exit(92);
    if (syscall(SYS_close, writer) != 0) _Exit(93);
}

ssize_t pread(int fd, void *buffer, size_t count, off_t offset) {
    if (!target_reader(fd)) return (ssize_t)syscall(SYS_pread64, fd, buffer, count, offset);
    if (selected_fd != fd || metadata_calls != 2 || count == 0) _Exit(94);
    read_calls++;
    if (read_calls == 1) {
        if (mode_is("pread-error") || mode_is("read-close-error") || mode_is("pread-unavailable")) {
            injections++; errno = mode_is("pread-unavailable") ? EAGAIN : EIO; return -1;
        }
        if (mode_is("pread-eintr")) { injections++; errno = EINTR; return -1; }
        if (mode_is("pread-eof")) { truncate_target(fd); injections++; }
    }
    if (mode_is("pread-short") || mode_is("expire-partial")) {
        if (count > 7) count = 7;
        if (read_calls == 1) injections++;
    }
    ssize_t result = (ssize_t)syscall(SYS_pread64, fd, buffer, count, offset);
    if (mode_is("expire-partial") && read_calls == 1) {
        if (result != 7) _Exit(95);
        expired = 1;
    }
    return result;
}

int close(int fd) {
    int selected = selected_fd == fd && target_reader(fd);
    int status = (int)syscall(SYS_close, fd);
    if (!selected) return status;
    close_calls++;
    if (status != 0) _Exit(96);
    /* All designated close failures consume the real descriptor first. The
     * destructor rejects a retry or leaked reader; no synthetic fd is retained. */
    if (mode_is("close-error") || mode_is("read-close-error")) {
        injections++; errno = EIO; return -1;
    }
    if (mode_is("expire-close")) { injections++; expired = 1; }
    return 0;
}

int clock_gettime(clockid_t clock, struct timespec *value) {
    int status = (int)syscall(SYS_clock_gettime, clock, value);
    if (status == 0 && clock == CLOCK_MONOTONIC && expired) value->tv_sec += 60;
    return status;
}

__attribute__((destructor)) static void verify_read_witness(void) {
    int expected_injections = mode_is("read-close-error") ? 2 : 1;
    int expected_reads = mode_is("metadata-error") ? 0 : mode_is("pread-eintr") ? 2 : 1;
    int reads_valid = mode_is("pread-short") ? read_calls > 1 : read_calls == expected_reads;
    int expected_expiry = mode_is("expire-partial") || mode_is("expire-close");
    if (selected_fd < 0 || metadata_calls != 2 || close_calls != 1 ||
        injections != expected_injections || !reads_valid || expired != expected_expiry) {
        fputs("FAIL: designated real cache read lifecycle fault was not exercised exactly\n", stderr);
        _Exit(97);
    }
    if (syscall(SYS_fcntl, selected_fd, F_GETFD) != -1 || errno != EBADF) {
        fputs("FAIL: cache reader descriptor survived consumption\n", stderr);
        _Exit(98);
    }
    fputs("PASS: designated cache read lifecycle boundary executed with closed reader\n", stderr);
}
