#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>

// Module: main
int64_t seen_main() {
    int64_t r0;
    char* name;
    char* message;
    message = "Hello, World!";
    name = "Seen";
    r0 = println(message);
    return r0;
}

int main(int argc, char* argv[]) {
    return seen_main();
}
