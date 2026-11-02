/* Test-only native CUDA forwarding plus bounded adverse capability replies. */
#include "cuda_staging_fault.c"

cudaError_t CUDARTAPI cudaGetDeviceCount(int *count) {
    NEXT(cudaGetDeviceCount); cudaError_t status = next(count);
    if (status == cudaSuccess) {
        if (mode("peer-count-negative")) *count = -1;
        if (mode("peer-count-zero")) *count = 0;
        if (mode("peer-count-limit")) *count = 129;
    }
    return status;
}
cudaError_t CUDARTAPI cudaDeviceCanAccessPeer(int *access, int device, int peer) {
    NEXT(cudaDeviceCanAccessPeer); cudaError_t status = next(access, device, peer);
    if (status == cudaSuccess) {
        if (mode("peer-access-invalid")) *access = 2;
        if (mode("peer-access-self")) *access = 1;
        if (mode("peer-query-error")) return cudaErrorUnknown;
        if (mode("peer-query-oom")) return cudaErrorMemoryAllocation;
    }
    return status;
}

cudaError_t CUDARTAPI cudaDeviceDisablePeerAccess(int peer) {
    NEXT(cudaDeviceDisablePeerAccess); cudaError_t status = next(peer);
    if (mode("peer-enable-invalid")) {
        /* The malformed owner targets an absent ordinal. Never invent a
         * successful enable or claim the injected 705 as physical cleanup. */
        if (status != cudaErrorInvalidDevice) abort();
        fprintf(stderr, "peer-negative actual-disable=%d injected-disable=705\n", (int)status);
        return cudaErrorPeerAccessNotEnabled;
    }
    return status;
}
