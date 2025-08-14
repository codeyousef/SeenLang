#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>

// Module: main
int64_t seen_main() {
    int64_t r0;
    int64_t r1;
    int64_t count;
    count = 0;
loop_start_0:
    r0 = count < 5;
    if (!r0) goto loop_end_2;
loop_body_1:
    r1 = count + 1;
    count = r1;
    goto loop_start_0;
loop_end_2:
    return count;
}

int main(int argc, char* argv[]) {
    return seen_main();
}
