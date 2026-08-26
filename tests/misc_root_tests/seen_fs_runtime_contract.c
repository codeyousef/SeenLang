#include "seen_runtime.h"

#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int32_t open_file(const char *path, int32_t flags, int32_t direct,
                         uint64_t *handle, uint64_t *alignment,
                         int32_t *active, uint64_t *fallbacks) {
    return seen_fs_open((int64_t)strlen(path), path, flags, direct, handle,
                        alignment, active, fallbacks);
}

static void join(char *output, size_t capacity, const char *root,
                 const char *name) {
    int written = snprintf(output, capacity, "%s/%s", root, name);
    assert(written > 0 && (size_t)written < capacity);
}

int main(int argc, char **argv) {
    assert(argc == 2);
    char directory[4096], original[4096], renamed[4096], link_path[4096];
    char direct_path[4096];
    join(directory, sizeof(directory), argv[1], "tree");
    join(original, sizeof(original), directory, "large.bin");
    join(renamed, sizeof(renamed), directory, "renamed.bin");
    join(link_path, sizeof(link_path), directory, "link.bin");
    join(direct_path, sizeof(direct_path), directory, "direct.bin");

    assert(seen_fs_create_directory((int64_t)strlen(directory), directory,
                                    0700) == SEEN_FS_OK);
    uint64_t file = 0, alignment = 0, fallbacks = 0;
    int32_t direct_active = 0;
    assert(open_file(original, 1 | 2 | 4 | 32 | 128, 0, &file, &alignment,
                     &direct_active, &fallbacks) == SEEN_FS_OK);
    assert(file != 0 && alignment >= 512 && direct_active == 0 && fallbacks == 0);
    uint64_t transferred = 0;
    uint8_t one = 0;
    assert(seen_fs_write_at(file, (uint64_t)INT64_MAX, &one, 1, 0, 1,
                            &transferred) == SEEN_FS_RANGE);
    assert(seen_fs_preallocate(file, (uint64_t)INT64_MAX, 1) ==
           SEEN_FS_RANGE);

    int64_t values[8] = {0, 1, 2, 3, 252, 253, 254, 255};
    SeenArray bytes = {8, 8, (int64_t)sizeof(int64_t), values};
    assert(seen_fs_pwrite_array(file, 0, &bytes, 0, 8, &transferred) ==
           SEEN_FS_OK && transferred == 8);
    int64_t marker[1] = {77};
    SeenArray marker_bytes = {1, 1, (int64_t)sizeof(int64_t), marker};
    const uint64_t large_offset = 5ULL * 1024ULL * 1024ULL * 1024ULL;
    assert(seen_fs_pwrite_array(file, large_offset, &marker_bytes, 0, 1,
                                &transferred) == SEEN_FS_OK && transferred == 1);
    assert(seen_fs_sync(file, 0) == SEEN_FS_OK);

    uint64_t size = 0, mode = 0, device = 0, inode = 0, links = 0;
    int32_t kind = 0;
    int64_t modified = 0;
    assert(seen_fs_metadata(file, &size, &kind, &mode, &modified, &device,
                            &inode, &links) == SEEN_FS_OK);
    assert(size == large_offset + 1 && kind == 1 && inode != 0 && links == 1);
    uint64_t available = 0, total = 0, block_size = 0;
    assert(seen_fs_space(file, &available, &total, &block_size) == SEEN_FS_OK);
    assert(total >= available && block_size > 0);
    assert(seen_fs_preallocate(file, 4096, 4096) == SEEN_FS_OK);
    int32_t punched = seen_fs_punch_hole(file, 4096, 4096);
    assert(punched == SEEN_FS_OK || punched == SEEN_FS_UNSUPPORTED);
    assert(seen_fs_lock(file, 1, 0) == SEEN_FS_OK);

    uint64_t second = 0, second_alignment = 0, second_fallbacks = 0;
    int32_t second_active = 0;
    assert(open_file(original, 1 | 2 | 128, 0, &second, &second_alignment,
                     &second_active, &second_fallbacks) == SEEN_FS_OK);
    assert(seen_fs_lock(second, 1, 0) == SEEN_FS_BUSY);
    assert(seen_fs_unlock(file) == SEEN_FS_OK);
    assert(seen_fs_lock(second, 1, 0) == SEEN_FS_OK);
    assert(seen_fs_unlock(second) == SEEN_FS_OK);
    assert(seen_fs_close(&second) == SEEN_FS_OK && second == 0);

    int64_t read_values[8] = {0};
    SeenArray read_bytes = {8, 8, (int64_t)sizeof(int64_t), read_values};
    assert(seen_fs_pread_array(file, 0, &read_bytes, 0, 8, &transferred) ==
           SEEN_FS_OK && transferred == 8);
    assert(memcmp(values, read_values, sizeof(values)) == 0);
    assert(seen_fs_close(&file) == SEEN_FS_OK && file == 0);

    assert(seen_fs_symlink((int64_t)strlen(original), original,
                           (int64_t)strlen(link_path), link_path) == SEEN_FS_OK);
    assert(seen_fs_path_kind((int64_t)strlen(link_path), link_path, &kind) ==
           SEEN_FS_OK && kind == 3);
    int32_t link_status = -1;
    SeenString target = seen_fs_readlink((int64_t)strlen(link_path), link_path,
                                         &link_status);
    assert(link_status == SEEN_FS_OK && target.len == (int64_t)strlen(original));
    assert(memcmp(target.data, original, (size_t)target.len) == 0);
    free(target.data);

    uint64_t direct_file = 0, direct_alignment = 0, direct_fallbacks = 0;
    int32_t required_active = 0;
    assert(open_file(direct_path, 1 | 2 | 4 | 32 | 128, 2, &direct_file,
                     &direct_alignment, &required_active, &direct_fallbacks) ==
           SEEN_FS_OK);
    assert(required_active == 1 && direct_fallbacks == 0 &&
           direct_alignment >= 512);
    int64_t *direct_values = calloc((size_t)direct_alignment, sizeof(int64_t));
    int64_t *direct_read = calloc((size_t)direct_alignment, sizeof(int64_t));
    assert(direct_values && direct_read);
    for (uint64_t i = 0; i < direct_alignment; ++i)
        direct_values[i] = (int64_t)(i & 255U);
    SeenArray direct_bytes = {(int64_t)direct_alignment,
        (int64_t)direct_alignment, (int64_t)sizeof(int64_t), direct_values};
    SeenArray direct_output = {(int64_t)direct_alignment,
        (int64_t)direct_alignment, (int64_t)sizeof(int64_t), direct_read};
    assert(seen_fs_pwrite_array(direct_file, 0, &direct_bytes, 0,
                                (int64_t)direct_alignment, &transferred) ==
           SEEN_FS_OK && transferred == direct_alignment);
    assert(seen_fs_pread_array(direct_file, 0, &direct_output, 0,
                               (int64_t)direct_alignment, &transferred) ==
           SEEN_FS_OK && transferred == direct_alignment);
    assert(memcmp(direct_values, direct_read,
                  (size_t)direct_alignment * sizeof(int64_t)) == 0);
    assert(seen_fs_pwrite_array(direct_file, 1, &marker_bytes, 0, 1,
                                &transferred) == SEEN_FS_ALIGNMENT);
    assert(seen_fs_close(&direct_file) == SEEN_FS_OK);
    free(direct_values);
    free(direct_read);

    uint64_t iterator = 0;
    assert(seen_fs_directory_open((int64_t)strlen(directory), directory,
                                  &iterator) == SEEN_FS_OK);
    int entries = 0;
    for (;;) {
        int32_t entry_status = -1, entry_kind = 0;
        SeenString name = seen_fs_directory_next(iterator, &entry_kind,
                                                 &entry_status);
        if (entry_status == SEEN_FS_END) break;
        assert(entry_status == SEEN_FS_OK && name.len > 0);
        ++entries;
        free(name.data);
    }
    assert(entries == 3);
    assert(seen_fs_directory_close(&iterator) == SEEN_FS_OK && iterator == 0);

    assert(seen_fs_rename((int64_t)strlen(original), original,
                          (int64_t)strlen(renamed), renamed, 1) == SEEN_FS_OK);
    assert(seen_fs_rename((int64_t)strlen(renamed), renamed,
                          (int64_t)strlen(link_path), link_path, 1) ==
           SEEN_FS_EXISTS);
    assert(seen_fs_remove_path((int64_t)strlen(link_path), link_path, 0) ==
           SEEN_FS_OK);
    assert(seen_fs_remove_path((int64_t)strlen(renamed), renamed, 0) ==
           SEEN_FS_OK);
    assert(seen_fs_remove_path((int64_t)strlen(direct_path), direct_path, 0) ==
           SEEN_FS_OK);
    assert(seen_fs_remove_path((int64_t)strlen(directory), directory, 1) ==
           SEEN_FS_OK);

    int32_t filesystem = -1;
    assert(seen_fs_filesystem_kind((int64_t)strlen(argv[1]), argv[1],
                                   &filesystem) == SEEN_FS_OK);
    assert(filesystem >= 0 && filesystem <= 2);
    puts("filesystem runtime contract passed");
    return 0;
}
