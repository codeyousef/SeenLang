#include <stdio.h>

int main(void) {
#if defined(__x86_64__) && defined(__linux__)
    __builtin_cpu_init();
    if (!__builtin_cpu_supports("x86-64-v3")) {
        fputs("test.002a.platform: native SIMD CLI execution requires x86-64-v3 CPU/OS support\n", stderr);
        return 77;
    }
    puts("PASS: native x86-64-v3 CPU/OS feature admission");
    return 0;
#else
    fputs("test.002a.platform: native SIMD CLI execution requires Linux x86-64\n", stderr);
    return 77;
#endif
}
