#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>

// Module: main
int64_t seen_main() {
    char* message;
    bool flag;
    int64_t number;
    number = 100;
    message = "world";
    flag = true;
    return number;
}

int main(int argc, char* argv[]) {
    return seen_main();
}
