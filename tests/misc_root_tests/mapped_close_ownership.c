#include "seen_runtime.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <windows.h>
#else
#include <unistd.h>
#endif

#define CHECK(expr) do { if (!(expr)) { \
    fprintf(stderr, "mapped close ownership failure at line %d\n", __LINE__); \
    return 1; \
} } while (0)

int main(int argc, char **argv) {
    CHECK(argc == 2);
    FILE *file_out = fopen(argv[1], "wb");
    CHECK(file_out != NULL);
    CHECK(fwrite("owner", 1, 5, file_out) == 5);
    CHECK(fclose(file_out) == 0);

    uint64_t file = 0;
    uint64_t length = 0;
    CHECK(seen_mapped_file_open_readonly((int64_t)strlen(argv[1]), argv[1],
                                         &file, &length) == SEEN_MMAP_OK);
    CHECK(file != 0 && length == 5);
    uint64_t window = 0;
    const uint8_t *data = NULL;
    CHECK(seen_mapped_file_window(file, 0, length, &window, &data) ==
          SEEN_MMAP_OK);
    CHECK(window != 0 && data != NULL);

    CHECK(seen_mapped_window_lock(window) == SEEN_MMAP_OK);
    uint64_t original_window = window;
    seen_mapped_test_set_close_failures(1, 0, 0);
    CHECK(seen_mapped_window_close(&window) == SEEN_MMAP_LOCK_FAILED);
    CHECK(window == original_window);

    seen_mapped_test_set_close_failures(0, 1, 0);
    CHECK(seen_mapped_window_close(&window) == SEEN_MMAP_MAP_FAILED);
    CHECK(window == original_window);
    CHECK(seen_mapped_file_close(&file) == SEEN_MMAP_BUSY);
    CHECK(file != 0);

    CHECK(seen_mapped_window_close(&window) == SEEN_MMAP_OK);
    CHECK(window == 0);
    CHECK(seen_mapped_window_close(&window) == SEEN_MMAP_OK);

    uint64_t original_file = file;
    seen_mapped_test_set_close_failures(0, 0, 1);
    CHECK(seen_mapped_file_close(&file) == SEEN_MMAP_STAT_FAILED);
    CHECK(file == original_file);
    CHECK(seen_mapped_file_close(&file) == SEEN_MMAP_OK);
    CHECK(file == 0);
    CHECK(seen_mapped_file_close(&file) == SEEN_MMAP_OK);
    return 0;
}
