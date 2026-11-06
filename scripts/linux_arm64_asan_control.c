#define _GNU_SOURCE
#include <sanitizer/asan_interface.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/personality.h>
#include <sys/resource.h>

/* Compiled afresh by the admission owner, never by the Seen fixture. */
int main(int argc, char **argv) {
    struct rlimit limit;
    size_t scale = 0, offset = 0;
    if (argc != 2) {
        fputs("ARM_ASAN_CONTROL expected one mode argument\n", stderr);
        return 2;
    }
    if (getrlimit(RLIMIT_AS, &limit)) {
        perror("ARM_ASAN_CONTROL getrlimit");
        return 2;
    }
    int persona = personality(0xffffffffUL);
    if (persona < 0) {
        perror("ARM_ASAN_CONTROL personality query");
        return 2;
    }
    if (persona & ADDR_NO_RANDOMIZE) {
        fputs("ARM_ASAN_CONTROL process has ASLR disabled\n", stderr);
        return 2;
    }
    __asan_get_shadow_mapping(&scale, &offset);
    printf("ARM_ASAN_CONTROL soft=%llu hard=%llu scale=%zu offset=%zu aslr_disabled=0\n",
           (unsigned long long)limit.rlim_cur, (unsigned long long)limit.rlim_max,
           scale, offset);
    fflush(stdout);
    if (!strcmp(argv[1], "clean")) {
        unsigned char *value = malloc(8);
        if (!value) {
            perror("ARM_ASAN_CONTROL clean allocation");
            return 3;
        }
        memset(value, 7, 8);
        if (value[7] != 7) return 4;
        free(value);
        FILE *maps = fopen("/proc/self/maps", "r");
        if (!maps) {
            perror("ARM_ASAN_CONTROL maps open");
            return 5;
        }
        char line[1024];
        while (fgets(line, sizeof(line), maps)) fputs(line, stdout);
        if (ferror(maps) || fclose(maps)) return 6;
        puts("ARM_ASAN_CONTROL clean-complete");
        return 0;
    }
    if (!strcmp(argv[1], "heap")) {
        volatile unsigned char *value = malloc(8);
        if (!value) return 3;
        value[8] = 9; /* A real instrumented one-byte heap-buffer overflow. */
        free((void *)value);
        return 7; /* Reaching this is a failed sanitizer, never acceptance. */
    }
    return 2;
}
