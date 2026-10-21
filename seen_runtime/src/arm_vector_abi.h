#ifndef SEEN_ARM_VECTOR_ABI_H
#define SEEN_ARM_VECTOR_ABI_H
#include <stdint.h>
/* 0 success, 1 invalid argument, 3 unavailable/interrupted, 4 unsupported. */
int32_t seen_arm_sve_vector_bytes(uint64_t *bytes);
#endif
