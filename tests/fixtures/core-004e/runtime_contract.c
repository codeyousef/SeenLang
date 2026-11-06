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

static int check_snapshot_views(int argc, char **argv) {
    int64_t baseline = seen_memory_used_bytes();
    int32_t status = -9;
    for (int iteration = 0; iteration < 1024; iteration++) {
        SeenString first = seen_deterministic_argument_view(0, &status);
        if (status != 0 || !string_is(first, argv[0]))
            return fail("first borrowed argument mismatch");
        SeenString last = seen_deterministic_argument_view(argc - 1, &status);
        if (status != 0 || !string_is(last, argv[argc - 1]))
            return fail("last borrowed argument mismatch");
        SeenString visible = seen_deterministic_environment_view(
            borrowed("CORE_004E_VISIBLE"), &status);
        if (status != 0 || !string_is(visible, "granted-value"))
            return fail("borrowed environment mismatch");
    }
    const int64_t invalid_indexes[] = {-1, INT64_MIN, argc, INT64_MAX};
    for (size_t index = 0;
         index < sizeof(invalid_indexes) / sizeof(invalid_indexes[0]); index++) {
        SeenString absent = seen_deterministic_argument_view(
            invalid_indexes[index], &status);
        if (status != 1 || absent.len != 0)
            return fail("missing borrowed argument status mismatch");
    }
    SeenString missing = seen_deterministic_environment_view(
        borrowed("CORE_004E_MISSING"), &status);
    if (status != 1 || missing.len != 0)
        return fail("missing borrowed environment status mismatch");
    const SeenString invalid_names[] = {
        {0, ""}, {1, NULL}, {INT64_MAX, "X"}, {5, "A\0BCD"}};
    for (size_t index = 0;
         index < sizeof(invalid_names) / sizeof(invalid_names[0]); index++) {
        SeenString invalid = seen_deterministic_environment_view(
            invalid_names[index], &status);
        if (status != -1 || invalid.len != 0)
            return fail("invalid borrowed environment name accepted");
    }
    if (seen_memory_used_bytes() != baseline)
        return fail("launch-snapshot views retained allocations");
    return 0;
}

int main(int argc, char **argv) {
    seen_runtime_init(argc, argv);
    if (argc == 2 && strcmp(argv[1], "--disabled-views") == 0) {
        if (seen_deterministic_runtime_status() != 0)
            return fail("disabled snapshot unexpectedly enabled");
        if (setenv("SEEN_DETERMINISTIC", "1", 1) != 0)
            return fail("could not prepare late-enable rejection");
        int32_t status = 0;
        SeenString argument = seen_deterministic_argument_view(0, &status);
        if (status != -1 || argument.len != 0)
            return fail("disabled argument view read ambient state");
        SeenString environment = seen_deterministic_environment_view(
            borrowed("SEEN_DETERMINISTIC"), &status);
        if (status != -1 || environment.len != 0)
            return fail("late environment change enabled snapshot views");
        return 0;
    }
    if (seen_deterministic_runtime_status() != 1) {
        return fail("valid deterministic snapshot was not enabled");
    }
    if (argc == 2 && strcmp(argv[1], "--empty-view") == 0) {
        int32_t status = -9;
        SeenString empty = seen_deterministic_environment_view(
            borrowed("CORE_004E_VISIBLE"), &status);
        if (status != 0 || empty.len != 0)
            return fail("present empty snapshot value became missing");
        return 0;
    }
    if (check_snapshot_views(argc, argv) != 0) return 1;
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

    int32_t view_status = -9;
    SeenString retained_argument = seen_deterministic_argument_view(
        1, &view_status);
    if (view_status != 0) return fail("argument view was unavailable");
    argv[1][0] = 'X';
    SeenArray *arguments = __GetCommandLineArgs();
    if (!arguments || arguments->len != 8 ||
        !string_is(seen_arr_get_str_ptr(arguments, 1),
            "captured-argument")) {
        return fail("argv was reread after runtime initialization");
    }
    if (!string_is(retained_argument, "captured-argument"))
        return fail("borrowed argument changed after ambient argv mutation");

    SeenString visible = borrowed("CORE_004E_VISIBLE");
    SeenString replacement = borrowed("replacement");
    if (!__HasEnv(visible) ||
        !string_is(__GetEnv(visible), "granted-value")) {
        return fail("captured environment value is unavailable");
    }
    SeenString retained_environment = seen_deterministic_environment_view(
        visible, &view_status);
    if (view_status != 0) return fail("environment view was unavailable");
    if (setenv("CORE_004E_VISIBLE", "ambient-mutation", 1) != 0 ||
        !string_is(__GetEnv(visible), "granted-value")) {
        return fail("environment was reread after runtime initialization");
    }
    if (!string_is(retained_environment, "granted-value"))
        return fail("borrowed environment changed after ambient mutation");
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
