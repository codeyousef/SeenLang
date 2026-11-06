#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/syscall.h>
#include <unistd.h>

static int failed_closes;
static int replacement = -1;

int close(int fd) {
    const char *target = getenv("SEEN_CLOSE_ERROR_PATH");
    char descriptor[64], path[PATH_MAX + 1];
    if (target && failed_closes == 0) {
        int length = snprintf(descriptor, sizeof descriptor, "/proc/self/fd/%d", fd);
        ssize_t count = length > 0 && (size_t)length < sizeof descriptor
                            ? readlink(descriptor, path, sizeof path - 1) : -1;
        if (count >= 0) {
            path[count] = '\0';
            if (strcmp(path, target) == 0) {
                if (syscall(SYS_close, fd) != 0) _Exit(90);
                replacement = (int)syscall(SYS_openat, AT_FDCWD, "/dev/null", O_RDONLY | O_CLOEXEC, 0);
                if (replacement != fd) _Exit(91);
                failed_closes = 1;
                errno = EINTR;
                return -1;
            }
        }
    }
    return (int)syscall(SYS_close, fd);
}

__attribute__((destructor)) static void verify_replacement_survived(void) {
    if (failed_closes != 1 || replacement < 0 || fcntl(replacement, F_GETFD) < 0) {
        fputs("FAIL: Seen exceptional close retried or did not exercise the reused descriptor\n", stderr);
        _Exit(92);
    }
    if (syscall(SYS_close, replacement) != 0) _Exit(93);
    fputs("PASS: Seen exceptional close preserved the reused descriptor\n", stderr);
}
