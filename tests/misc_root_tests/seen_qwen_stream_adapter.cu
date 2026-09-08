#include "seen_qwen_stream_adapter.h"

#include <cuda_runtime_api.h>

#include <cstdint>

namespace {

__global__ void increment_i32(int32_t *values, uint64_t count) {
    const uint64_t index = static_cast<uint64_t>(blockIdx.x) * blockDim.x +
        threadIdx.x;
    if (index < count) values[index] += 1;
}

SeenCudaStatus adapter_status(int32_t code, int32_t native_code,
                              int32_t device, const char *message) {
    SeenCudaStatus result{};
    result.code = code;
    result.native_code = native_code;
    result.device_ordinal = device;
    result.maturity = SEEN_CUDA_MATURITY_EXPERIMENTAL_HARDWARE;
    result.operation = "seen-qwen-test-increment-i32";
    result.message = message;
    return result;
}

}  // namespace

extern "C" SeenCudaStatus seen_qwen_test_increment_i32(SeenCudaHandle stream,
    int32_t expected_device, int32_t *device_values, uint64_t count,
    uint32_t *observed_token_flags) {
    if (!device_values || !observed_token_flags || count == 0 ||
        count > UINT32_MAX)
        return adapter_status(SEEN_CUDA_INVALID_ARGUMENT, 0, expected_device,
                              "invalid bounded Qwen adapter launch");
    SeenCudaStreamLaunchToken token{};
    SeenCudaStatus borrowed = seen_cuda_stream_borrow_launch_token(
        stream, expected_device, &token);
    if (borrowed.code != SEEN_CUDA_OK) return borrowed;
    if (token.abi_version != SEEN_CUDA_STREAM_LAUNCH_TOKEN_ABI_VERSION ||
        token.native_stream == 0 || token.device_ordinal != expected_device)
        return adapter_status(SEEN_CUDA_INCOMPATIBLE, 0, expected_device,
                              "incompatible CUDA stream launch token");
    *observed_token_flags = token.flags;
    cudaStream_t native_stream = reinterpret_cast<cudaStream_t>(
        static_cast<uintptr_t>(token.native_stream));
    constexpr uint32_t threads = 128;
    const uint32_t blocks = static_cast<uint32_t>((count + threads - 1) / threads);
    increment_i32<<<blocks, threads, 0, native_stream>>>(device_values, count);
    cudaError_t error = cudaGetLastError();
    return error == cudaSuccess
        ? adapter_status(SEEN_CUDA_OK, 0, expected_device, "ok")
        : adapter_status(SEEN_CUDA_RUNTIME_ERROR, static_cast<int32_t>(error),
                         expected_device, cudaGetErrorString(error));
}
