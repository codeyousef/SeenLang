#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>

// Module: main
int64_t seen_main() {
    int64_t r0;
    int64_t r1;
    int64_t r2;
    int64_t sum;
    int64_t i;
    i = 1;
for_start_0:
    r0 = i <= 5;
    if (!r0) goto for_end_2;
for_body_1:
    r1 = sum + i;
    sum = r1;
    r2 = i + 1;
    i = r2;
    goto for_start_0;
for_end_2:
    return sum;
}

int main(int argc, char* argv[]) {
    return seen_main();
}
