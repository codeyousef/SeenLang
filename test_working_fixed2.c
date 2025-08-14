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
    int64_t r4;
    int64_t r5;
    int64_t r6;
    int64_t r7;
    int64_t r8;
    int64_t r9;
    int64_t counter;
    int64_t x;
    int64_t sum;
    int64_t y;
    x = 10;
    y = 20;
    r0 = x + y;
    sum = r0;
    counter = 0;
    goto loop_start_0;
    r1 = counter < 5;
    if (r1) goto loop_body_1;
    r2 = counter + 1;
    counter = r2;
    goto loop_start_0;
    goto loop_end_2;
    goto for_end_5;
    r4 = total + i;
    total = r4;
    r5 = i + 1;
    i = r5;
    goto for_start_3;
    numbers = (int64_t*)malloc(5 * sizeof(int64_t));
    numbers[0] = 1;
    numbers[1] = 2;
    numbers[2] = 3;
    numbers[3] = 4;
    numbers[4] = 5;
    r6 = numbers[0];
    first = r6;
    r7 = sum + total;
    r8 = r7 + counter;
    r9 = r8 + first;
    return r9;
    r3 = i <= 5;
    if (r3) goto for_body_4;
    total = 0;
    i = 1;
    goto for_start_3;
}

int main(int argc, char* argv[]) {
    return seen_main();
}
