#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>

// Module: main
int64_t seen_main() {
    int64_t r0;
    int64_t r1;
    int64_t r2;
    int64_t second;
    int64_t first;
    int64_t* arr;
    arr = (int64_t*)malloc(5 * sizeof(int64_t));
    arr[0] = 1;
    arr[1] = 2;
    arr[2] = 3;
    arr[3] = 4;
    arr[4] = 5;
    r0 = arr[0];
    first = r0;
    r1 = arr[1];
    second = r1;
    r2 = first + second;
    return r2;
}

int main(int argc, char* argv[]) {
    return seen_main();
}
