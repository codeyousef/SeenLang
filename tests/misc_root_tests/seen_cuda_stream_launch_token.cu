#include "seen_cuda.h"
#include "seen_qwen_stream_adapter.h"

#include <cstdint>
#include <cstdio>
#include <cstring>

#define CHECK_STATUS(expr) do { SeenCudaStatus s_ = (expr); if (s_.code != 0) { \
    std::fprintf(stderr, "FAIL:%d: %s code=%d native=%d op=%s message=%s\n", \
        __LINE__, #expr, s_.code, s_.native_code, s_.operation, s_.message); \
    return 1; } } while (0)
#define CHECK(expr) do { if (!(expr)) { \
    std::fprintf(stderr, "FAIL:%d: %s\n", __LINE__, #expr); return 1; \
} } while (0)

int main() {
    constexpr uint64_t count = 1024;
    constexpr uint64_t bytes = count * sizeof(int32_t);
    SeenCudaHandle stream = 0, event = 0, allocation = 0, host = 0;
    CHECK_STATUS(seen_cuda_stream_create(0, &stream));
    CHECK_STATUS(seen_cuda_event_create(0, &event));
    CHECK_STATUS(seen_cuda_malloc(0, bytes, &allocation));
    CHECK_STATUS(seen_cuda_host_alloc(bytes, &host));

    void *device_data = nullptr, *host_data = nullptr;
    uint64_t actual_bytes = 0;
    int32_t actual_device = -1;
    CHECK_STATUS(seen_cuda_allocation_address(allocation, &device_data,
        &actual_bytes, &actual_device));
    CHECK(actual_bytes == bytes && actual_device == 0);
    CHECK_STATUS(seen_cuda_host_allocation_address(host, &host_data,
        &actual_bytes));
    CHECK(actual_bytes == bytes);

    SeenCudaStreamLaunchToken token{};
    CHECK_STATUS(seen_cuda_stream_borrow_launch_token(stream, 0, &token));
    CHECK(token.abi_version == SEEN_CUDA_STREAM_LAUNCH_TOKEN_ABI_VERSION);
    CHECK(token.flags & SEEN_CUDA_STREAM_LAUNCH_CAPTURE_COMPATIBLE);
    CHECK(!(token.flags & SEEN_CUDA_STREAM_LAUNCH_CAPTURE_ACTIVE));
    CHECK(token.device_ordinal == 0 && token.native_stream != 0 &&
          token.generation != 0 && token.reserved == 0);

    SeenCudaStreamLaunchToken rejected;
    std::memset(&rejected, 0xff, sizeof(rejected));
    SeenCudaStatus zero = seen_cuda_stream_borrow_launch_token(0, 0, &rejected);
    CHECK(zero.code == SEEN_CUDA_CLOSED && rejected.native_stream == 0);
    SeenCudaStatus invalid = seen_cuda_stream_borrow_launch_token(
        UINT64_C(0x1234), 0, &rejected);
    CHECK(invalid.code == SEEN_CUDA_INVALID_ARGUMENT && rejected.native_stream == 0);
    SeenCudaStatus cross_device = seen_cuda_stream_borrow_launch_token(
        stream, 1, &rejected);
    CHECK(cross_device.code == SEEN_CUDA_INCOMPATIBLE &&
          cross_device.device_ordinal == 0 && rejected.native_stream == 0);
    SeenCudaStatus negative_device = seen_cuda_stream_borrow_launch_token(
        stream, -1, &rejected);
    CHECK(negative_device.code == SEEN_CUDA_INVALID_ARGUMENT &&
          rejected.native_stream == 0);
    SeenCudaStatus missing_output = seen_cuda_stream_borrow_launch_token(
        stream, 0, nullptr);
    CHECK(missing_output.code == SEEN_CUDA_INVALID_ARGUMENT);

    SeenCudaHandle temporary_stream = 0;
    CHECK_STATUS(seen_cuda_stream_create(0, &temporary_stream));
    SeenCudaStreamLaunchToken temporary_token{};
    CHECK_STATUS(seen_cuda_stream_borrow_launch_token(
        temporary_stream, 0, &temporary_token));
    SeenCudaHandle stale_stream = temporary_stream;
    CHECK_STATUS(seen_cuda_stream_destroy(&temporary_stream));
    SeenCudaHandle replacement_stream = 0;
    CHECK_STATUS(seen_cuda_stream_create(0, &replacement_stream));
    SeenCudaStreamLaunchToken replacement_token{};
    CHECK_STATUS(seen_cuda_stream_borrow_launch_token(
        replacement_stream, 0, &replacement_token));
    CHECK(replacement_token.generation != temporary_token.generation);
    SeenCudaStatus stale = seen_cuda_stream_borrow_launch_token(
        stale_stream, 0, &rejected);
    CHECK(stale.code == SEEN_CUDA_CLOSED && rejected.native_stream == 0);
    CHECK_STATUS(seen_cuda_stream_destroy(&replacement_stream));

    auto *host_values = static_cast<int32_t *>(host_data);
    for (uint64_t index = 0; index < count; ++index)
        host_values[index] = static_cast<int32_t>(index);
    CHECK_STATUS(seen_cuda_memcpy_async(device_data, host_data, bytes,
        SEEN_CUDA_COPY_HOST_TO_DEVICE, stream));
    uint32_t observed_flags = 0;
    CHECK_STATUS(seen_qwen_test_increment_i32(stream, 0,
        static_cast<int32_t *>(device_data), count, &observed_flags));
    CHECK(observed_flags & SEEN_CUDA_STREAM_LAUNCH_CAPTURE_COMPATIBLE);
    CHECK(!(observed_flags & SEEN_CUDA_STREAM_LAUNCH_CAPTURE_ACTIVE));
    CHECK_STATUS(seen_cuda_memcpy_async(host_data, device_data, bytes,
        SEEN_CUDA_COPY_DEVICE_TO_HOST, stream));
    CHECK_STATUS(seen_cuda_event_record(event, stream));
    CHECK_STATUS(seen_cuda_event_synchronize(event));
    for (uint64_t index = 0; index < count; ++index)
        CHECK(host_values[index] == static_cast<int32_t>(index + 1));

    CHECK_STATUS(seen_cuda_graph_begin_capture(stream));
    observed_flags = 0;
    CHECK_STATUS(seen_qwen_test_increment_i32(stream, 0,
        static_cast<int32_t *>(device_data), count, &observed_flags));
    CHECK(observed_flags & SEEN_CUDA_STREAM_LAUNCH_CAPTURE_ACTIVE);
    SeenCudaHandle graph = 0, graph_exec = 0;
    CHECK_STATUS(seen_cuda_graph_end_capture(stream, &graph));
    CHECK_STATUS(seen_cuda_graph_instantiate(graph, &graph_exec));
    CHECK_STATUS(seen_cuda_graph_launch(graph_exec, stream));
    CHECK_STATUS(seen_cuda_memcpy_async(host_data, device_data, bytes,
        SEEN_CUDA_COPY_DEVICE_TO_HOST, stream));
    CHECK_STATUS(seen_cuda_event_record(event, stream));
    CHECK_STATUS(seen_cuda_event_synchronize(event));
    for (uint64_t index = 0; index < count; ++index)
        CHECK(host_values[index] == static_cast<int32_t>(index + 2));

    CHECK_STATUS(seen_cuda_graph_exec_destroy(&graph_exec));
    CHECK_STATUS(seen_cuda_graph_destroy(&graph));
    CHECK_STATUS(seen_cuda_host_free(&host));
    CHECK_STATUS(seen_cuda_free(&allocation));
    CHECK_STATUS(seen_cuda_event_destroy(&event));
    SeenCudaHandle closed_stream = stream;
    CHECK_STATUS(seen_cuda_stream_destroy(&stream));
    SeenCudaStatus after_close = seen_cuda_stream_borrow_launch_token(
        closed_stream, 0, &rejected);
    CHECK(after_close.code == SEEN_CUDA_CLOSED && rejected.native_stream == 0);
    std::printf("PASS: borrowed CUDA stream token ordering, capture, diagnostics, and teardown\n");
    return 0;
}
