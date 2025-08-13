#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>

// Module: main
int64_t seen_main() {
    int64_t r0;
    int64_t r1;
    int64_t r2;
    int64_t r3;
    int64_t x;
    x = 50;
    if (x > 10) {
        r3 = x + 10;
    } else {
        r3 = x - 10;
    }
    return r3;
    return r2;
}

int main(int argc, char* argv[]) {
    return seen_main();
}
