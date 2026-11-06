/* Independent of Seen codegen/runtime and the staging owner. This diagnostic
 * never overrides a failed gate: it identifies native SDK startup failures. */
#include <cuda_runtime_api.h>
#include <stdio.h>
int main(void) {
    cudaError_t status = cudaSetDevice(0);
    fprintf(stderr, "DIRECT_CUDA_INIT native=%d message=%s\n", (int)status, cudaGetErrorString(status));
    return status == cudaSuccess ? 0 : 2;
}
