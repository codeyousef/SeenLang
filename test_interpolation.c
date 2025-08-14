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
    int64_t name;
    int64_t greeting;
    name = "World";
    r0 = "";
    r1 = (char*)malloc(1024); sprintf(r1, "%s%s", r0, "Hello, ");
    r2 = (char*)malloc(1024); sprintf(r2, "%s%s", r1, name);
    r3 = (char*)malloc(1024); sprintf(r3, "%s%s", r2, "!");
    greeting = r3;
    return greeting;
}

int main(int argc, char* argv[]) {
    return seen_main();
}
