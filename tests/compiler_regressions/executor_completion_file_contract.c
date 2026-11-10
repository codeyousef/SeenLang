#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include "seen_runtime.h"
#include "src/executor_completion_abi.h"
#include <assert.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
int main(void) {
    char path[] = "completion-file.XXXXXX";
    int temporary = mkstemp(path); assert(temporary >= 0); assert(close(temporary) == 0);
    uint64_t handle = 0, alignment = 0, fallbacks = 0; int32_t active = 0;
    assert(seen_fs_open((int64_t)strlen(path), path, 3 | 128, 0, &handle,
        &alignment, &active, &fallbacks) == SEEN_FS_OK);
    int64_t duplicate = -1;
    assert(seen_async_completion_duplicate_file(handle, &duplicate) == 4 && duplicate == -1);
    assert(seen_fs_close(&handle) == SEEN_FS_OK && handle == 0);
    assert(seen_fs_open((int64_t)strlen(path), path, 3 | 128, 2, &handle,
        &alignment, &active, &fallbacks) == SEEN_FS_OK && active == 1 && fallbacks == 0);
    assert(seen_async_completion_duplicate_file(handle, &duplicate) == 0 && duplicate >= 3);
    assert(fcntl((int)duplicate, F_GETFD) & FD_CLOEXEC);
    assert(fcntl((int)duplicate, F_GETFL) & O_DIRECT);
    assert(seen_fs_close(&handle) == SEEN_FS_OK && handle == 0);
    void *buffer = NULL; assert(posix_memalign(&buffer, 4096, 4096) == 0); memset(buffer, 42, 4096);
    assert(pwrite((int)duplicate, buffer, 4096, 0) == 4096);
    assert(close((int)duplicate) == 0); free(buffer);
    assert(unlink(path) == 0);
    return 0;
}
