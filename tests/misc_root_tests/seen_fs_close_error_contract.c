#define _GNU_SOURCE
#include "seen_runtime.h"
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>

static int fail_next_close;
static int replacement = -1;
static unsigned attempted;
int __real_close(int descriptor);
int __wrap_close(int descriptor) {
    ++attempted;
    int result = __real_close(descriptor);
    if (fail_next_close) {
        fail_next_close = 0;
        assert(result == 0);
        replacement = open("/dev/null", O_RDONLY | O_CLOEXEC);
        assert(replacement == descriptor);
        errno = EINTR;
        return -1;
    }
    return result;
}
int main(int argc, char **argv) {
    assert(argc == 2);
    int64_t baseline = seen_memory_used_bytes();
    uint64_t handle = 0, alignment = 0, fallbacks = 0;
    int32_t active = 0;
    assert(seen_fs_open((int64_t)strlen(argv[1]), argv[1], 1 | 2 | 4 | 32 | 128,
        0, &handle, &alignment, &active, &fallbacks) == SEEN_FS_OK);
    fail_next_close = 1;
    assert(seen_fs_close(&handle) == SEEN_FS_IO_FAILED);
    assert(handle == 0);
    assert(attempted == 1 && replacement >= 0);
    assert(seen_memory_used_bytes() == baseline);
    assert(seen_fs_close(&handle) == SEEN_FS_OK);
    assert(attempted == 1 && fcntl(replacement, F_GETFD) >= 0);
    assert(__real_close(replacement) == 0);
    return 0;
}
