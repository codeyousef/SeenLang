#define _POSIX_C_SOURCE 200809L

#include "seen_runtime.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern SeenArray *__GetCommandLineArgs(void);
extern bool __HasEnv(SeenString name);
extern SeenString __GetEnv(SeenString name);
extern bool __SetEnv(SeenString name, SeenString value);
extern bool __RemoveEnv(SeenString name);

static SeenString borrowed(const char *value) {
    SeenString result = {(int64_t)strlen(value), (char *)value};
    return result;
}

static int string_is(SeenString actual, const char *expected) {
    size_t length = strlen(expected);
    return actual.len == (int64_t)length && actual.data != NULL &&
        memcmp(actual.data, expected, length) == 0;
}

static int fail(const char *detail) {
    fprintf(stderr, "CORE-004E runtime contract failed: %s\n", detail);
    return 1;
}

int main(int argc, char **argv) {
    seen_runtime_init(argc, argv);
    if (seen_deterministic_runtime_status() != 1) {
        return fail("valid deterministic snapshot was not enabled");
    }
    if (argc == 257 && strcmp(argv[1], "--max-arguments") == 0) {
        SeenArray *maximum_arguments = __GetCommandLineArgs();
        if (!maximum_arguments || maximum_arguments->len != 257 ||
            !string_is(seen_arr_get_str_ptr(maximum_arguments, 1),
                "--max-arguments") ||
            !string_is(seen_arr_get_str_ptr(maximum_arguments, 256), "a")) {
            return fail("maximum application-argument snapshot mismatch");
        }
        puts("PASS: CORE-004E maximum application-argument snapshot");
        return 0;
    }
    if (argc != 8) return fail("expected fixture paths and one captured arg");

    if (seen_time_system_nanos() != INT64_C(1700000000000000000)) {
        return fail("wall clock did not use the captured epoch");
    }

    argv[1][0] = 'X';
    SeenArray *arguments = __GetCommandLineArgs();
    if (!arguments || arguments->len != 8 ||
        !string_is(seen_arr_get_str_ptr(arguments, 1),
            "captured-argument")) {
        return fail("argv was reread after runtime initialization");
    }

    SeenString visible = borrowed("CORE_004E_VISIBLE");
    SeenString replacement = borrowed("replacement");
    if (!__HasEnv(visible) ||
        !string_is(__GetEnv(visible), "granted-value")) {
        return fail("captured environment value is unavailable");
    }
    if (setenv("CORE_004E_VISIBLE", "ambient-mutation", 1) != 0 ||
        !string_is(__GetEnv(visible), "granted-value")) {
        return fail("environment was reread after runtime initialization");
    }
    if (__SetEnv(visible, replacement) || __RemoveEnv(visible)) {
        return fail("deterministic environment mutation was accepted");
    }

    SeenString root = borrowed(argv[2]);
    SeenString child = borrowed(argv[3]);
    SeenString sibling = borrowed(argv[4]);
    SeenString escape = borrowed(argv[5]);
    SeenString inside_link = borrowed(argv[6]);
    SeenString missing = borrowed(argv[7]);
    if (seen_deterministic_path_beneath(root, root) != 1 ||
        seen_deterministic_path_beneath(root, child) != 1) {
        return fail("equal or direct child path was denied");
    }
    if (seen_deterministic_path_beneath(root, sibling) != 0) {
        return fail("sibling prefix path was accepted");
    }
    if (seen_deterministic_path_beneath(root, escape) != 0) {
        return fail("symlink escape was accepted");
    }
    if (seen_deterministic_path_beneath(root, inside_link) != 1) {
        return fail("symlink resolving inside the root was denied");
    }
    if (seen_deterministic_path_beneath(root, missing) != -1) {
        return fail("missing path did not report canonicalization failure");
    }

    puts("PASS: CORE-004E bounded runtime snapshot and path containment");
    return 0;
}
