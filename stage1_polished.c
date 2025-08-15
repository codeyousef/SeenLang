#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>

// Module: main
bool CompileProgram() {
    char* r1;
    char* r2;
    int64_t r3;
    char* sourceCode;
    int64_t r0;
    int64_t cCode;
    r0 = "";
    r1 = (char*)malloc(1024); sprintf(r1, "%s%s", r0, "fun hello() -> Int ");
    return 42;
block_0:
    r2 = (char*)malloc(1024); sprintf(r2, "%s%d", r1, 42);
    sourceCode = r2;
    r3 = GenerateCCode(sourceCode);
    cCode = r3;
    return true;
block_1:
    return true;
}

char* GenerateCCode(char* source) {
    int64_t r0;
    char* r2;
    char* r1;
    r0 = "";
    r1 = (char*)malloc(1024); sprintf(r1, "%s%s", r0, "#include <stdio.h>\nint main() ");
    return 42;
block_0:
    r2 = (char*)malloc(1024); sprintf(r2, "%s%d", r1, 42);
    return r2;
block_1:
    return r2;
}

int64_t seen_main() {
    return 0;
}

int main(int argc, char* argv[]) {
    return seen_main();
}
