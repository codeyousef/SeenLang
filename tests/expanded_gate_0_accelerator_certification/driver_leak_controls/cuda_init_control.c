/* SDK-only CUDA leak baseline: cuInit alone, no Seen code (FEL-1111). */
#include <cuda.h>
#include <stdio.h>

int main(void) {
    CUresult status = cuInit(0);
    printf("cuInit=%d\n", (int)status);
    return status == CUDA_SUCCESS ? 0 : 1;
}
