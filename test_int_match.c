#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>

// Module: main
int64_t seen_main() {
    int64_t r0;
    int64_t r1;
    int64_t r2;
    r1 = 42 == 1;
    if (r1) goto match_arm_0_0;
match_arm_0_0:
    r0 = 100;
    goto match_end_3;
match_end_3:
    return r0;
block_0:
    r2 = 42 == 42;
    if (r2) goto match_arm_1_1;
block_1:
    goto match_arm_2_2;
match_arm_1_1:
    r0 = 999;
    goto match_end_3;
match_arm_2_2:
    r0 = 0;
    goto match_end_3;
}

int main(int argc, char* argv[]) {
    return seen_main();
}
