#define _FILE_OFFSET_BITS 64
#define _GNU_SOURCE
#include "seen_runtime.h"
#include "seen_inference.h"

#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

#define CHECK(expr) do { if (!(expr)) { \
    fprintf(stderr, "FAIL:%d: %s\n", __LINE__, #expr); return 1; \
} } while (0)

int main(int argc, char **argv) {
    CHECK(sizeof(SeenFloat16) == 2 && _Alignof(SeenFloat16) == 2);
    CHECK(sizeof(SeenBFloat16) == 2 && _Alignof(SeenBFloat16) == 2);
    CHECK(sizeof(SeenFloat8E4M3) == 1 && sizeof(SeenFloat8E5M2) == 1);
    CHECK(argc == 2);
    const char *path = argv[1];
    const uint64_t file_size = UINT64_C(60) * 1024 * 1024 * 1024;
    const uint64_t marker_offset = UINT64_C(5) * 1024 * 1024 * 1024 + 37;
    int fd = open(path, O_CREAT | O_EXCL | O_RDWR | O_CLOEXEC, 0600);
    CHECK(fd >= 0);
    CHECK(ftruncate(fd, (off_t)file_size) == 0);
    const uint8_t marker[] = {0x51, 0x77, 0x65, 0x6e};
    CHECK(pwrite(fd, marker, sizeof(marker), (off_t)marker_offset) ==
          (ssize_t)sizeof(marker));
    CHECK(close(fd) == 0);

    uint64_t file = 0, length = 0;
    CHECK(seen_mapped_file_open_readonly((int64_t)strlen(path), path,
                                         &file, &length) == SEEN_MMAP_OK);
    CHECK(file != 0 && length == file_size);
    uint64_t window = 0;
    const uint8_t *data = NULL;
    CHECK(seen_mapped_file_window(file, UINT64_MAX - 1, 8,
                                  &window, &data) == SEEN_MMAP_RANGE);
    CHECK(window == 0 && data == NULL);
    CHECK(seen_mapped_file_window(file, marker_offset, sizeof(marker),
                                  &window, &data) == SEEN_MMAP_OK);
    CHECK(window != 0 && data != NULL && memcmp(data, marker, sizeof(marker)) == 0);
    SeenString borrowed = seen_string_view_bytes(data, (int64_t)sizeof(marker));
    CHECK(borrowed.len == (int64_t)sizeof(marker) && borrowed.data == (char *)data);
    CHECK(seen_mapped_window_advise(window, 1) == SEEN_MMAP_OK);
    CHECK(seen_mapped_window_advise(window, 2) == SEEN_MMAP_OK);
    CHECK(seen_mapped_window_advise(window, 3) == SEEN_MMAP_OK);
    int32_t huge_status = seen_mapped_window_advise(window, 4);
    CHECK(huge_status == SEEN_MMAP_OK || huge_status == SEEN_MMAP_UNSUPPORTED);
    CHECK(seen_mapped_window_lock(window) == SEEN_MMAP_OK);
    CHECK(seen_mapped_window_lock(window) == SEEN_MMAP_OK);
    CHECK(seen_mapped_window_unlock(window) == SEEN_MMAP_OK);
    CHECK(seen_mapped_window_unlock(window) == SEEN_MMAP_OK);
    int32_t numa_status = seen_mapped_window_bind_numa(window, 0);
    CHECK(numa_status == SEEN_MMAP_OK || numa_status == SEEN_MMAP_NUMA_FAILED ||
          numa_status == SEEN_MMAP_UNSUPPORTED);
    CHECK(seen_mapped_file_close(&file) == SEEN_MMAP_BUSY);

    fd = open(path, O_WRONLY | O_CLOEXEC);
    CHECK(fd >= 0);
    CHECK(ftruncate(fd, (off_t)(marker_offset + 1)) == 0);
    CHECK(close(fd) == 0);
    CHECK(seen_mapped_window_validate(window) == SEEN_MMAP_TRUNCATED);
    CHECK(seen_mapped_window_close(&window) == SEEN_MMAP_OK && window == 0);
    CHECK(seen_mapped_window_close(&window) == SEEN_MMAP_OK);
    CHECK(seen_mapped_file_length(file, &length) == SEEN_MMAP_TRUNCATED);
    CHECK(seen_mapped_file_close(&file) == SEEN_MMAP_OK && file == 0);
    CHECK(seen_mapped_file_close(&file) == SEEN_MMAP_OK);
    const int64_t alignments[] = {16, 32, 64, 128, 256};
    for (size_t index = 0; index < sizeof(alignments) / sizeof(alignments[0]); ++index) {
        void *aligned = seen_try_aligned_realloc(NULL, 0, 4096, alignments[index]);
        CHECK(aligned != NULL &&
              ((uintptr_t)aligned % (uintptr_t)alignments[index]) == 0U);
        seen_aligned_buffer_free(aligned, 4096, alignments[index]);
    }
    CHECK(unlink(path) == 0);
    puts("PASS: 60 GiB sparse mappings, paging controls, alignment, and truncation safety");
    return 0;
}
