#include "seen_runtime.h"

#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

extern int64_t __OpenFile(SeenString path, SeenString mode);
extern int64_t __CloseFile(int64_t fd);
extern bool __FileExists(SeenString path);
extern bool __DeleteFile(SeenString path);
extern bool __CreateDirectory(SeenString path);
extern bool seen_replace_file(SeenString source, SeenString destination);
extern int64_t __ExecuteProgram(SeenString path);
extern int64_t __ExecuteProgramRequest(SeenString path,
    SeenString request_path);
extern bool __HasEnv(SeenString name);
extern SeenString __GetEnv(SeenString name);
extern bool __SetEnv(SeenString name, SeenString value);
extern bool __RemoveEnv(SeenString name);
extern int32_t seen_deterministic_path_beneath(SeenString root,
    SeenString path);

static SeenString view(const char *value) {
    SeenString result = {(int64_t)strlen(value), (char *)value};
    return result;
}

static void fail(const char *label, int64_t before_live,
    int64_t after_live, int64_t before_reserve, int64_t after_reserve) {
    fprintf(stderr, "FAIL: %s live=%lld/%lld reserve=%lld/%lld\n", label,
        (long long)before_live, (long long)after_live,
        (long long)before_reserve, (long long)after_reserve);
    exit(1);
}

static void require_balanced(const char *label, int64_t before_live,
    int64_t before_reserve) {
    int64_t after_live = seen_memory_used_bytes();
    int64_t after_reserve = seen_memory_reserved_bytes();
    if (after_live != before_live || after_reserve != before_reserve) {
        fail(label, before_live, after_live, before_reserve, after_reserve);
    }
}

static void release_checked_string(SeenString value) {
    if (!value.data) return;
    seen_memory_release_bytes(value.len + 1);
    free(value.data);
}

static void write_raw_file(const char *path, const char *contents) {
    FILE *file = fopen(path, "wb");
    if (!file) {
        fprintf(stderr, "FAIL: could not create %s: %s\n", path,
            strerror(errno));
        exit(1);
    }
    size_t length = strlen(contents);
    if (fwrite(contents, 1, length, file) != length || fclose(file) != 0) {
        fprintf(stderr, "FAIL: could not write %s\n", path);
        exit(1);
    }
}

int main(int argc, char **argv) {
    if (argc != 2 || chdir(argv[1]) != 0) {
        fprintf(stderr, "runtime C-string fixture requires a writable work directory\n");
        return 64;
    }
    const char *unicode_file = "runtime-cstring-\xC3\xA9.txt";
    const char *unicode_directory = "runtime-cstring-\xE7\x9B\xAE\xE5\xBD\x95";
    const char *missing = "runtime-cstring-missing";
    const char *environment_name = "SEEN_RUNTIME_CSTRING_ACCOUNTING_TEST";
    char long_name[241];
    memset(long_name, 'a', sizeof(long_name) - 1);
    long_name[sizeof(long_name) - 1] = 0;

    write_raw_file(unicode_file, "fixture");
    write_raw_file(long_name, "long");

    int64_t baseline_live = seen_memory_used_bytes();
    int64_t baseline_reserve = seen_memory_reserved_bytes();

    for (int iteration = 0; iteration < 8; iteration++) {
        int64_t fd = __OpenFile(view(unicode_file), view("rb"));
        if (fd < 0 || __CloseFile(fd) != 0) return 2;
        require_balanced("file open success", baseline_live, baseline_reserve);

        if (__OpenFile(view(missing), view("rb")) >= 0) return 3;
        require_balanced("file open failure", baseline_live, baseline_reserve);

        if (__OpenFile(view(""), view("r")) >= 0) return 4;
        require_balanced("empty file path", baseline_live, baseline_reserve);

        if (!__FileExists(view(unicode_file)) ||
            __FileExists(view(missing)) || !__FileExists(view(long_name))) {
            return 5;
        }
        require_balanced("file existence paths", baseline_live,
            baseline_reserve);

        if (iteration == 0) {
            if (!__CreateDirectory(view(unicode_directory))) return 6;
        } else if (__CreateDirectory(view(unicode_directory))) {
            return 7;
        }
        require_balanced("directory success and failure", baseline_live,
            baseline_reserve);

        if (__ExecuteProgram(view("true")) != 0) return 8;
        if (__ExecuteProgramRequest(view("/bin/true"), view(missing)) != 0) {
            return 9;
        }
        require_balanced("process paths", baseline_live, baseline_reserve);

        if (__HasEnv(view(environment_name))) return 10;
        if (!__SetEnv(view(environment_name), view("value"))) return 11;
        if (!__HasEnv(view(environment_name))) return 12;
        SeenString value = __GetEnv(view(environment_name));
        if (value.len != 5 || memcmp(value.data, "value", 5) != 0) return 13;
        release_checked_string(value);
        if (!__RemoveEnv(view(environment_name))) return 14;
        if (__HasEnv(view(environment_name)) || __HasEnv(view(""))) return 15;
        require_balanced("environment names and values", baseline_live,
            baseline_reserve);

        if (seen_deterministic_path_beneath(view("."), view(".")) != 1 ||
            seen_deterministic_path_beneath(view("."), view(missing)) != -1) {
            return 16;
        }
        require_balanced("deterministic path resolution", baseline_live,
            baseline_reserve);
    }

    write_raw_file("runtime-cstring-source", "replace");
    if (!seen_replace_file(view("runtime-cstring-source"),
            view("runtime-cstring-destination"))) return 17;
    require_balanced("replace file success", baseline_live, baseline_reserve);
    if (seen_replace_file(view(missing), view("runtime-cstring-destination"))) {
        return 18;
    }
    require_balanced("replace file failure", baseline_live, baseline_reserve);
    if (!__DeleteFile(view("runtime-cstring-destination")) ||
        __DeleteFile(view(missing))) return 19;
    require_balanced("delete file success and failure", baseline_live,
        baseline_reserve);

    unlink(unicode_file);
    unlink(long_name);
    rmdir(unicode_directory);
    printf("PASS: runtime C-string callers restore live accounting\n");
    return 0;
}
