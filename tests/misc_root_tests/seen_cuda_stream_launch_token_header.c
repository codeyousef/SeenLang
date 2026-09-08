#include "seen_cuda.h"

#include <stddef.h>

_Static_assert(sizeof(SeenCudaStreamLaunchToken) == 32,
               "launch token must retain its fixed-width ABI");
_Static_assert(offsetof(SeenCudaStreamLaunchToken, abi_version) == 0,
               "unexpected abi_version offset");
_Static_assert(offsetof(SeenCudaStreamLaunchToken, flags) == 4,
               "unexpected flags offset");
_Static_assert(offsetof(SeenCudaStreamLaunchToken, device_ordinal) == 8,
               "unexpected device offset");
_Static_assert(offsetof(SeenCudaStreamLaunchToken, native_stream) == 16,
               "unexpected native stream offset");
_Static_assert(offsetof(SeenCudaStreamLaunchToken, generation) == 24,
               "unexpected generation offset");

int main(void) {
    SeenCudaStreamLaunchToken token = {0};
    return SEEN_CUDA_STREAM_LAUNCH_TOKEN_ABI_VERSION == 1u &&
           SEEN_CUDA_STREAM_LAUNCH_CAPTURE_COMPATIBLE == 1u &&
           SEEN_CUDA_STREAM_LAUNCH_CAPTURE_ACTIVE == 2u &&
           token.native_stream == 0 ? 0 : 1;
}
