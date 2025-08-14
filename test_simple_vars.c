#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>

// Module: main
int64_t seen_main() {
    int64_t r0;
    int64_t x;
    int64_t y;
    x = 10;
    y = 20;
    r0 = x + y;
    return r0;
}

int main(int argc, char* argv[]) {
    return seen_main();
}
