#include <assert.h>
#include <errno.h>
#include <stdint.h>
#include "../../seen_runtime/src/arm_vector_abi.h"
static int value;
static int error;
static int calls;
int __wrap_prctl(int option, ...) {
    assert(option == 51); ++calls; errno = error; return value;
}
int main(void) {
    uint64_t bytes = 7;
    assert(seen_arm_sve_vector_bytes(0) == 1 && calls == 0);
    value = 16; error = 0;
    assert(seen_arm_sve_vector_bytes(&bytes) == 0 && bytes == 16);
    value = 32;
    assert(seen_arm_sve_vector_bytes(&bytes) == 0 && bytes == 32);
    value = 256 | (1 << 17);
    assert(seen_arm_sve_vector_bytes(&bytes) == 0 && bytes == 256);
    value = -1; error = EINVAL;
    assert(seen_arm_sve_vector_bytes(&bytes) == 4 && bytes == 0);
    error = EPERM;
    assert(seen_arm_sve_vector_bytes(&bytes) == 4 && bytes == 0);
    error = EINTR;
    assert(seen_arm_sve_vector_bytes(&bytes) == 3 && bytes == 0);
    assert(calls == 6);
    return 0;
}
