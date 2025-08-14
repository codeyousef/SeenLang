#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>

// Struct type definitions
typedef struct {
    int64_t x;
    int64_t y;
} Point;

// Module: main
int64_t seen_main() {
    int64_t r0;
    int64_t r1;
    int64_t r2;
    Point p;
    p = (Point){.x = 10, .y = 20};
    r0 = p.x;
    r1 = p.y;
    r2 = r0 + r1;
    return r2;
}

int main(int argc, char* argv[]) {
    return seen_main();
}
