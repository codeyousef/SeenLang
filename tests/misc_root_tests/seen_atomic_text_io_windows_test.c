#include "seen_runtime.h"

#include <windows.h>

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

enum {
    WRITER_COUNT = 8,
    WRITES_PER_THREAD = 16,
    CONTENT_CAPACITY = 4096
};

static void fail(const char* message) {
    fprintf(stderr, "Windows atomic text I/O test failed: %s\n", message);
    ExitProcess(1);
}

static void require_true(int condition, const char* message) {
    if (!condition) fail(message);
}

static SeenString seen_string(const char* value) {
    SeenString result = {(int64_t)strlen(value), (char*)value};
    return result;
}

static void setup_file(const char* path, const char* content) {
    FILE* file = fopen(path, "wb");
    if (!file) fail("could not create setup file");
    size_t length = strlen(content);
    if (fwrite(content, 1, length, file) != length || fflush(file) != 0 ||
        fclose(file) != 0) {
        fail("could not write setup file");
    }
}

static char* read_file(const char* path, size_t* result_length) {
    FILE* file = fopen(path, "rb");
    if (!file || fseek(file, 0, SEEK_END) != 0) {
        fail("could not open or seek result file");
    }
    long end = ftell(file);
    if (end < 0 || fseek(file, 0, SEEK_SET) != 0) {
        fail("could not size result file");
    }
    char* content = (char*)malloc((size_t)end + 1U);
    if (!content) fail("result allocation failed");
    size_t count = fread(content, 1, (size_t)end, file);
    if (count != (size_t)end || ferror(file) || fclose(file) != 0) {
        free(content);
        fail("could not read complete result file");
    }
    content[count] = '\0';
    *result_length = count;
    return content;
}

static void require_file_content(const char* path, const char* expected) {
    size_t actual_length = 0;
    char* actual = read_file(path, &actual_length);
    size_t expected_length = strlen(expected);
    int matches = actual_length == expected_length &&
        memcmp(actual, expected, expected_length) == 0;
    free(actual);
    require_true(matches, "destination content did not match");
}

static int temporary_residue_count(const char* allowed_leaf) {
    WIN32_FIND_DATAA data;
    HANDLE search = FindFirstFileA(".seen-tmp.*", &data);
    if (search == INVALID_HANDLE_VALUE) {
        return GetLastError() == ERROR_FILE_NOT_FOUND ? 0 : -1;
    }
    int count = 0;
    do {
        if (!allowed_leaf || strcmp(data.cFileName, allowed_leaf) != 0) {
            count++;
        }
    } while (FindNextFileA(search, &data));
    DWORD error = GetLastError();
    FindClose(search);
    return error == ERROR_NO_MORE_FILES ? count : -1;
}

static void require_no_temporary_residue(void) {
    require_true(temporary_residue_count(NULL) == 0,
        "runtime left a temporary file behind");
}

typedef struct {
    char content[CONTENT_CAPACITY];
    volatile LONG failed;
} Writer;

static DWORD WINAPI writer_main(LPVOID raw_writer) {
    Writer* writer = (Writer*)raw_writer;
    for (int iteration = 0; iteration < WRITES_PER_THREAD; iteration++) {
        if (!seen_write_text_atomic(seen_string("destination.txt"),
                seen_string(writer->content))) {
            InterlockedExchange(&writer->failed, 1);
            break;
        }
    }
    return 0;
}

static void test_concurrent_writers(void) {
    Writer writers[WRITER_COUNT];
    HANDLE threads[WRITER_COUNT];
    for (int writer = 0; writer < WRITER_COUNT; writer++) {
        writers[writer].failed = 0;
        int prefix = snprintf(writers[writer].content,
            sizeof(writers[writer].content), "writer-%02d:", writer);
        if (prefix <= 0 || (size_t)prefix >= sizeof(writers[writer].content)) {
            fail("writer prefix overflow");
        }
        size_t cursor = (size_t)prefix;
        while (cursor + 2U < sizeof(writers[writer].content)) {
            writers[writer].content[cursor++] = (char)('A' + writer);
        }
        writers[writer].content[cursor++] = '\n';
        writers[writer].content[cursor] = '\0';
        threads[writer] = CreateThread(NULL, 0, writer_main,
            &writers[writer], 0, NULL);
        require_true(threads[writer] != NULL,
            "could not create writer thread");
    }
    DWORD wait = WaitForMultipleObjects(WRITER_COUNT, threads, TRUE, 60000);
    require_true(wait >= WAIT_OBJECT_0 &&
        wait < WAIT_OBJECT_0 + WRITER_COUNT,
        "could not join writer threads");
    for (int writer = 0; writer < WRITER_COUNT; writer++) {
        CloseHandle(threads[writer]);
        require_true(writers[writer].failed == 0,
            "a concurrent atomic write failed");
    }

    size_t actual_length = 0;
    char* actual = read_file("destination.txt", &actual_length);
    int complete_writer = 0;
    for (int writer = 0; writer < WRITER_COUNT; writer++) {
        size_t expected_length = strlen(writers[writer].content);
        if (actual_length == expected_length &&
            memcmp(actual, writers[writer].content, expected_length) == 0) {
            complete_writer = 1;
            break;
        }
    }
    free(actual);
    require_true(complete_writer,
        "concurrent writers produced torn or combined output");
    require_no_temporary_residue();
}

int main(void) {
    require_true(seen_write_text_atomic(seen_string("destination.txt"),
            seen_string("created")), "initial atomic create failed");
    require_file_content("destination.txt", "created");
    require_true(seen_write_text_atomic(seen_string("destination.txt"),
            seen_string("")), "empty atomic replacement failed");
    require_file_content("destination.txt", "");
    require_true(seen_write_text_atomic(seen_string("destination.txt"),
            seen_string("replacement")), "existing replacement failed");
    require_file_content("destination.txt", "replacement");

    const int failure_stages[] = {
        SEEN_ATOMIC_IO_FAIL_WRITE,
        SEEN_ATOMIC_IO_FAIL_SYNC,
        SEEN_ATOMIC_IO_FAIL_CLOSE,
        SEEN_ATOMIC_IO_FAIL_RENAME
    };
    for (size_t i = 0; i < sizeof(failure_stages) /
            sizeof(failure_stages[0]); i++) {
        setup_file("destination.txt", "preserve-on-failure");
        seen_test_atomic_write_fail_stage(failure_stages[i]);
        int wrote = seen_write_text_atomic(seen_string("destination.txt"),
            seen_string("must-not-commit"));
        seen_test_atomic_write_fail_stage(SEEN_ATOMIC_IO_FAIL_NONE);
        require_true(!wrote, "injected operation failure reported success");
        require_file_content("destination.txt", "preserve-on-failure");
        require_no_temporary_residue();
    }

    const char* collision_leaf =
        ".seen-tmp.00000000000000000000000000000000";
    setup_file(collision_leaf, "pre-existing-collision");
    seen_test_atomic_write_force_collisions(1);
    require_true(seen_write_text_atomic(seen_string("destination.txt"),
            seen_string("collision-retry")),
        "exclusive temporary collision was not retried");
    require_file_content("destination.txt", "collision-retry");
    require_file_content(collision_leaf, "pre-existing-collision");
    require_true(temporary_residue_count(collision_leaf) == 0,
        "collision retry left an unowned temporary file");

    HANDLE destination_lock = CreateFileA("destination.txt", GENERIC_READ,
        FILE_SHARE_READ, NULL, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, NULL);
    require_true(destination_lock != INVALID_HANDLE_VALUE,
        "could not lock destination against replacement");
    require_true(!seen_write_text_atomic(seen_string("destination.txt"),
            seen_string("must-not-replace-locked-file")),
        "locked replacement unexpectedly succeeded");
    CloseHandle(destination_lock);
    require_file_content("destination.txt", "collision-retry");
    require_true(temporary_residue_count(collision_leaf) == 0,
        "locked replacement left a temporary file");

    require_true(CreateDirectoryA("directory-destination", NULL),
        "could not create directory destination");
    require_true(!seen_write_text_atomic(seen_string("directory-destination"),
            seen_string("must-fail")),
        "directory destination was accepted");
    require_true(!seen_write_text_atomic(
            seen_string("missing-parent\\result.txt"),
            seen_string("must-fail")), "missing parent was accepted");

    setup_file("symlink-target.txt", "symlink-target-content");
    DWORD symlink_flags = 0x2U;
    BOOL symlink_created = CreateSymbolicLinkA("symlink-destination.txt",
        "symlink-target.txt", symlink_flags);
    if (!symlink_created && GetLastError() == ERROR_INVALID_PARAMETER) {
        symlink_created = CreateSymbolicLinkA("symlink-destination.txt",
            "symlink-target.txt", 0);
    }
    if (symlink_created) {
        require_true(!seen_write_text_atomic(
                seen_string("symlink-destination.txt"),
                seen_string("must-not-follow")),
            "atomic output followed a destination symlink");
        require_file_content("symlink-target.txt", "symlink-target-content");
        DWORD attributes = GetFileAttributesA("symlink-destination.txt");
        require_true(attributes != INVALID_FILE_ATTRIBUTES &&
            (attributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0,
            "destination reparse point was replaced");
    } else {
        DWORD error = GetLastError();
        require_true(error == ERROR_PRIVILEGE_NOT_HELD ||
            error == ERROR_NOT_SUPPORTED,
            "unexpected destination symlink creation failure");
        fprintf(stderr,
            "SKIP: Windows destination symlink policy (error %lu)\n",
            (unsigned long)error);
    }

    DeleteFileA(collision_leaf);
    require_no_temporary_residue();
    test_concurrent_writers();
    setup_file("windows-test-passed.marker", "passed");
    puts("Windows atomic text I/O runtime tests passed");
    return 0;
}
