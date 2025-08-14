#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>

// Module: main
int64_t seen_main() {
    int64_t r0;
    int64_t r1;
    r1 = 42 == 1;
    if (r1) goto match_arm_0_0;
match_arm_0_0:
    r0 = "one";
    goto match_end_1;
match_end_1:
    return r0;
block_0:
    goto match_end_1;
}

int main(int argc, char* argv[]) {
    return seen_main();
}
