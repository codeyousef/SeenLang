#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>

// Module: main
int64_t seen_main() {
    int64_t r0;
    int64_t r1;
    int64_t r2;
    r0 = 5 + 3;
    r1 = 10 - 2;
    r2 = r0 * r1;
    return r2;
}

int main(int argc, char* argv[]) {
    return seen_main();
}
