#define _POSIX_C_SOURCE 200809L

#include "seen_runtime.h"

#include <dirent.h>
#include <errno.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>

enum {
    TEST_PATH_CAPACITY = 4096,
    TEST_WRITER_COUNT = 12,
    TEST_WRITES_PER_THREAD = 32,
    TEST_CONTENT_CAPACITY = 8192
};

static void fail(const char* message) {
    fprintf(stderr, "atomic text I/O test failed: %s\n", message);
    exit(1);
}

static void require_true(int condition, const char* message) {
    if (!condition) fail(message);
}

static SeenString seen_string(const char* value) {
    SeenString result = {(int64_t)strlen(value), (char*)value};
    return result;
}

static void join_path(char* output, size_t capacity, const char* directory,
    const char* leaf) {
    int length = snprintf(output, capacity, "%s/%s", directory, leaf);
    if (length < 0 || (size_t)length >= capacity) fail("test path too long");
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
    if (!file) fail("could not open result file");
    if (fseek(file, 0, SEEK_END) != 0) fail("could not seek result file");
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

static int temporary_residue_count(const char* directory,
    const char* allowed_leaf) {
    DIR* stream = opendir(directory);
    if (!stream) fail("could not inspect test directory");
    int count = 0;
    struct dirent* entry;
    while ((entry = readdir(stream)) != NULL) {
        if (strncmp(entry->d_name, ".seen-tmp.", 10) == 0 &&
            (!allowed_leaf || strcmp(entry->d_name, allowed_leaf) != 0)) {
            count++;
        }
    }
    if (closedir(stream) != 0) fail("could not close test directory");
    return count;
}

static void require_no_temporary_residue(const char* directory) {
    require_true(temporary_residue_count(directory, NULL) == 0,
        "runtime left a temporary file behind");
}

typedef struct {
    const char* destination;
    char content[TEST_CONTENT_CAPACITY];
    int failed;
} Writer;

static void* writer_main(void* raw_writer) {
    Writer* writer = (Writer*)raw_writer;
    for (int iteration = 0; iteration < TEST_WRITES_PER_THREAD; iteration++) {
        if (!seen_write_text_atomic(seen_string(writer->destination),
                seen_string(writer->content))) {
            writer->failed = 1;
            break;
        }
    }
    return NULL;
}

static void test_concurrent_writers(const char* directory,
    const char* destination) {
    Writer writers[TEST_WRITER_COUNT];
    pthread_t threads[TEST_WRITER_COUNT];
    for (int writer = 0; writer < TEST_WRITER_COUNT; writer++) {
        writers[writer].destination = destination;
        writers[writer].failed = 0;
        int prefix = snprintf(writers[writer].content,
            sizeof(writers[writer].content), "writer-%02d:", writer);
        if (prefix <= 0 || (size_t)prefix >= sizeof(writers[writer].content)) {
            fail("writer prefix overflow");
        }
        char fill = (char)('A' + writer);
        size_t cursor = (size_t)prefix;
        while (cursor + 2U < sizeof(writers[writer].content)) {
            writers[writer].content[cursor++] = fill;
        }
        writers[writer].content[cursor++] = '\n';
        writers[writer].content[cursor] = '\0';
        if (pthread_create(&threads[writer], NULL, writer_main,
                &writers[writer]) != 0) {
            fail("could not create writer thread");
        }
    }
    for (int writer = 0; writer < TEST_WRITER_COUNT; writer++) {
        if (pthread_join(threads[writer], NULL) != 0) {
            fail("could not join writer thread");
        }
        require_true(!writers[writer].failed,
            "a concurrent atomic write failed");
    }

    size_t actual_length = 0;
    char* actual = read_file(destination, &actual_length);
    int complete_writer = 0;
    for (int writer = 0; writer < TEST_WRITER_COUNT; writer++) {
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
    require_no_temporary_residue(directory);
}

int main(int argc, char** argv) {
    if (argc != 2) fail("expected one project-local test directory");
    const char* directory = argv[1];
    char destination[TEST_PATH_CAPACITY];
    join_path(destination, sizeof(destination), directory, "destination.txt");

    require_true(seen_write_text_atomic(seen_string(destination),
            seen_string("created")), "initial atomic create failed");
    require_file_content(destination, "created");
    require_no_temporary_residue(directory);

    require_true(seen_write_text_atomic(seen_string(destination),
            seen_string("")), "empty atomic replacement failed");
    require_file_content(destination, "");
    require_no_temporary_residue(directory);

    require_true(chmod(destination, 0640) == 0,
        "could not set destination mode");
    require_true(seen_write_text_atomic(seen_string(destination),
            seen_string("replacement")), "existing replacement failed");
    require_file_content(destination, "replacement");
    struct stat destination_info;
    require_true(stat(destination, &destination_info) == 0 &&
        (destination_info.st_mode & 0777) == 0640,
        "replacement did not preserve destination mode");

    const int failure_stages[] = {
        SEEN_ATOMIC_IO_FAIL_WRITE,
        SEEN_ATOMIC_IO_FAIL_SYNC,
        SEEN_ATOMIC_IO_FAIL_CLOSE,
        SEEN_ATOMIC_IO_FAIL_RENAME
    };
    for (size_t i = 0; i < sizeof(failure_stages) /
            sizeof(failure_stages[0]); i++) {
        setup_file(destination, "preserve-on-failure");
        seen_test_atomic_write_fail_stage(failure_stages[i]);
        int wrote = seen_write_text_atomic(seen_string(destination),
            seen_string("must-not-commit"));
        seen_test_atomic_write_fail_stage(SEEN_ATOMIC_IO_FAIL_NONE);
        require_true(!wrote, "injected operation failure reported success");
        require_file_content(destination, "preserve-on-failure");
        require_no_temporary_residue(directory);
    }

    const char* collision_leaf =
        ".seen-tmp.00000000000000000000000000000000";
    char collision_path[TEST_PATH_CAPACITY];
    join_path(collision_path, sizeof(collision_path), directory,
        collision_leaf);
    setup_file(collision_path, "pre-existing-collision");
    seen_test_atomic_write_force_collisions(1);
    require_true(seen_write_text_atomic(seen_string(destination),
            seen_string("collision-retry")),
        "exclusive temporary collision was not retried");
    require_file_content(destination, "collision-retry");
    require_file_content(collision_path, "pre-existing-collision");
    require_true(temporary_residue_count(directory, collision_leaf) == 0,
        "collision retry left an unowned temporary file");

    char guessed_path[TEST_PATH_CAPACITY];
    int guessed_length = snprintf(guessed_path, sizeof(guessed_path),
        "%s.tmp.%ld", destination, (long)getpid());
    require_true(guessed_length > 0 &&
        (size_t)guessed_length < sizeof(guessed_path),
        "guessed path overflow");
    setup_file(guessed_path, "unowned-marker");
    require_true(seen_write_text_atomic(seen_string(destination),
            seen_string("does-not-touch-guesses")),
        "write beside guessed marker failed");
    require_file_content(guessed_path, "unowned-marker");

    char symlink_target[TEST_PATH_CAPACITY];
    char symlink_destination[TEST_PATH_CAPACITY];
    join_path(symlink_target, sizeof(symlink_target), directory,
        "symlink-target.txt");
    join_path(symlink_destination, sizeof(symlink_destination), directory,
        "symlink-destination.txt");
    setup_file(symlink_target, "symlink-target-content");
    require_true(symlink(symlink_target, symlink_destination) == 0,
        "could not create destination symlink");
    require_true(!seen_write_text_atomic(seen_string(symlink_destination),
            seen_string("must-not-follow")),
        "atomic output followed a destination symlink");
    require_file_content(symlink_target, "symlink-target-content");
    struct stat symlink_info;
    require_true(lstat(symlink_destination, &symlink_info) == 0 &&
        S_ISLNK(symlink_info.st_mode), "destination symlink was replaced");

    char directory_destination[TEST_PATH_CAPACITY];
    join_path(directory_destination, sizeof(directory_destination), directory,
        "directory-destination");
    require_true(mkdir(directory_destination, 0700) == 0,
        "could not create directory destination");
    require_true(!seen_write_text_atomic(seen_string(directory_destination),
            seen_string("must-fail")),
        "directory destination was accepted");

    char missing_destination[TEST_PATH_CAPACITY];
    join_path(missing_destination, sizeof(missing_destination), directory,
        "missing-parent/result.txt");
    require_true(!seen_write_text_atomic(seen_string(missing_destination),
            seen_string("must-fail")), "missing parent was accepted");

    char locked_directory[TEST_PATH_CAPACITY];
    char locked_destination[TEST_PATH_CAPACITY];
    join_path(locked_directory, sizeof(locked_directory), directory, "locked");
    join_path(locked_destination, sizeof(locked_destination), locked_directory,
        "destination.txt");
    require_true(mkdir(locked_directory, 0700) == 0,
        "could not create locked parent");
    setup_file(locked_destination, "locked-original");
    require_true(chmod(locked_directory, 0555) == 0,
        "could not make parent unwritable");
    int locked_write = seen_write_text_atomic(seen_string(locked_destination),
        seen_string("must-not-replace"));
    require_true(chmod(locked_directory, 0700) == 0,
        "could not restore locked parent mode");
    require_true(!locked_write, "unwritable parent was accepted");
    require_file_content(locked_destination, "locked-original");

    require_true(unlink(symlink_destination) == 0,
        "could not remove owned destination symlink");
    require_true(unlink(collision_path) == 0,
        "could not remove owned collision marker");
    require_true(unlink(guessed_path) == 0,
        "could not remove owned guessed marker");
    require_no_temporary_residue(directory);

    test_concurrent_writers(directory, destination);
    puts("atomic text I/O runtime tests passed");
    return 0;
}
