#include "arm_vector_abi.h"
#if defined(__linux__)
#include <errno.h>
#include <sys/prctl.h>
#ifndef PR_SVE_GET_VL
#define PR_SVE_GET_VL 51
#endif
#ifndef PR_SVE_VL_LEN_MASK
#define PR_SVE_VL_LEN_MASK 0xffff
#endif
#endif

int32_t seen_arm_sve_vector_bytes(uint64_t *bytes) {
    if (!bytes) return 1;
    *bytes = 0;
#if defined(__linux__)
    int value = prctl(PR_SVE_GET_VL, 0L, 0L, 0L, 0L);
    if (value < 0) {
        if (errno == EINVAL || errno == ENOSYS || errno == EPERM || errno == EACCES) return 4;
        return 3;
    }
    *bytes = (uint64_t)(value & PR_SVE_VL_LEN_MASK);
    return 0;
#else
    return 4;
#endif
}
