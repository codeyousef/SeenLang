#include "seen_cuda.h"

/* The opt-in completion accessor must not expose SDK headers or C long/bool
 * widths to CPU-only consumers. sizeof/_Generic emit no CUDA symbol call. */
_Static_assert(_Generic(&seen_cuda_event_query,
    SeenCudaStatus (*)(SeenCudaHandle, int32_t *): 1, default: 0),
    "event query must use the fixed-width existing resource ABI");
int main(void) { return SEEN_CUDA_ABI_VERSION == 1u ? 0 : 1; }
