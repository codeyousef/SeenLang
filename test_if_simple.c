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
    r0 = x > 42;
    then_0:
    r1 = x + 10;
    r2 = r1;
    else_1:
    r3 = x - 10;
    r2 = r3;
    if_end_2:
    return r2;
}

int main(int argc, char* argv[]) {
    return seen_main();
}
