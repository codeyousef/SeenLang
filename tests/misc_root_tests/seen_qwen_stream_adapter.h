#ifndef SEEN_QWEN_STREAM_ADAPTER_H
#define SEEN_QWEN_STREAM_ADAPTER_H

#include "seen_cuda.h"

#ifdef __cplusplus
extern "C" {
#endif

SeenCudaStatus seen_qwen_test_increment_i32(SeenCudaHandle stream,
    int32_t expected_device, int32_t *device_values, uint64_t count,
    uint32_t *observed_token_flags);

#ifdef __cplusplus
}
#endif

#endif
