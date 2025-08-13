#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>

// Module: main
int64_t seen_main() {
    int64_t r0;
    r0 = 42 + 10;
    return r0;
}

int main(int argc, char* argv[]) {
    return seen_main();
}
