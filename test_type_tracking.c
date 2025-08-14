#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>

// Module: main
int64_t seen_main() {
    int64_t num;
    char* text;
    num = 42;
    text = "hello";
    return num;
}

int main(int argc, char* argv[]) {
    return seen_main();
}
