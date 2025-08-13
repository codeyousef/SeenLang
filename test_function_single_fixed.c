#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>

// Module: main
int64_t add(int64_t a, int64_t b) {
    int64_t r0;
    r0 = a + b;
    return r0;
}

int64_t seen_main() {
    int64_t r0;
    r0 = add(5, 3);
    return r0;
}

int main(int argc, char* argv[]) {
    return seen_main();
}
