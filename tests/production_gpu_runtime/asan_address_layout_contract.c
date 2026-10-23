#define _GNU_SOURCE
#include <sanitizer/asan_interface.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>

static volatile unsigned char global_bytes[8];
__attribute__((noinline)) static void release(void *value) { free(value); }
__attribute__((noinline)) static void leak(void) {
    void *value = malloc(37);
    if (!value) abort();
    memset(value, 7, 37);
    __asm__ volatile("" : : "r"(value) : "memory");
}
__attribute__((noinline)) static int stack_access(size_t index) {
    volatile unsigned char bytes[8] = {0};
    return bytes[index];
}
int main(int argc, char **argv) {
    if (argc != 2) return 2;
    if (!strcmp(argv[1], "clean")) {
        unsigned char *value = malloc(8);
        if (!value) return 3;
        memset(value, 7, 8); release(value);
    } else if (!strcmp(argv[1], "heap")) {
        volatile unsigned char *value = malloc(8);
        if (!value) return 3;
        value[8] = 9;
        release((void *)value);
    } else if (!strcmp(argv[1], "use-after-free")) {
        volatile unsigned char *value = malloc(8);
        if (!value) return 3;
        release((void *)value);
        value[0] = 9;
    } else if (!strcmp(argv[1], "stack")) {
        return stack_access((size_t)argc + 6);
    } else if (!strcmp(argv[1], "global")) {
        global_bytes[(size_t)argc + 6] = 9;
    } else if (!strcmp(argv[1], "leak")) {
        leak();
    } else if (!strcmp(argv[1], "gap-clean") || !strcmp(argv[1], "gap-poison")) {
        /* Linux x86-64 default ASan shadow gap. The explicit CUDA layout
         * supplies shadow for valid application mappings here as well. */
        void *wanted = (void *)(uintptr_t)UINT64_C(0x10000000000);
        void *mapped = mmap(wanted, 4096, PROT_READ | PROT_WRITE,
                            MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED_NOREPLACE, -1, 0);
        if (mapped == MAP_FAILED) { perror("shadow-gap application mmap"); return 4; }
        if (mapped != wanted) return 5;
        volatile unsigned char *value = mapped;
        value[0] = 17;
        if (!strcmp(argv[1], "gap-poison")) {
            __asan_poison_memory_region(mapped, 8);
            value[0] = 9;
        }
        if (value[0] != 17 || munmap(mapped, 4096)) return 6;
    } else return 2;
    puts("PASS: ASan address-layout control");
    return 0;
}
