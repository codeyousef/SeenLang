#include "seen_cuda.h"

#include <cuda_bf16.h>
#include <cuda_fp16.h>

#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <vector>

#define CHECK_STATUS(expr) do { SeenCudaStatus s_ = (expr); if (s_.code != 0) { \
    std::fprintf(stderr, "FAIL:%d: %s native=%d op=%s message=%s\n", __LINE__, \
        #expr, s_.native_code, s_.operation, s_.message); return 1; } } while (0)
#define CHECK(expr) do { if (!(expr)) { \
    std::fprintf(stderr, "FAIL:%d: %s\n", __LINE__, #expr); return 1; \
} } while (0)

template <typename T> T one();
template <> __half one<__half>() { return __float2half(1.0f); }
template <> __nv_bfloat16 one<__nv_bfloat16>() { return __float2bfloat16(1.0f); }
template <typename T> float as_float(T value);
template <> float as_float(__half value) { return __half2float(value); }
template <> float as_float(__nv_bfloat16 value) { return __bfloat162float(value); }

template <typename T>
int matmul_case(SeenCudaHandle lt, SeenCudaHandle stream, int type) {
    constexpr uint64_t side = 16;
    constexpr uint64_t count = side * side;
    constexpr uint64_t bytes = count * sizeof(T);
    std::vector<T> host_a(count, one<T>()), host_b(count, one<T>()), host_c(count);
    SeenCudaHandle a = 0, b = 0, c = 0, workspace = 0;
    CHECK_STATUS(seen_cuda_malloc(0, bytes, &a));
    CHECK_STATUS(seen_cuda_malloc(0, bytes, &b));
    CHECK_STATUS(seen_cuda_malloc(0, bytes, &c));
    void *a_ptr = nullptr, *b_ptr = nullptr, *c_ptr = nullptr;
    uint64_t ignored_bytes = 0; int32_t ignored_device = -1;
    CHECK_STATUS(seen_cuda_allocation_address(a, &a_ptr, &ignored_bytes, &ignored_device));
    CHECK_STATUS(seen_cuda_allocation_address(b, &b_ptr, &ignored_bytes, &ignored_device));
    CHECK_STATUS(seen_cuda_allocation_address(c, &c_ptr, &ignored_bytes, &ignored_device));
    CHECK_STATUS(seen_cuda_memcpy_async(a_ptr, host_a.data(), bytes,
        SEEN_CUDA_COPY_HOST_TO_DEVICE, stream));
    CHECK_STATUS(seen_cuda_memcpy_async(b_ptr, host_b.data(), bytes,
        SEEN_CUDA_COPY_HOST_TO_DEVICE, stream));
    SeenCudaMatmulDesc desc{SEEN_CUDA_ABI_VERSION, type, 0, 0,
        side, side, side, (int64_t)side, (int64_t)side, (int64_t)side,
        UINT64_C(16) * 1024 * 1024};
    SeenCudaAlgorithm algorithm{};
    CHECK_STATUS(seen_cublaslt_select_algorithm(lt, &desc, &algorithm));
    void *workspace_ptr = nullptr;
    if (algorithm.workspace_bytes) {
        CHECK_STATUS(seen_cuda_malloc(0, algorithm.workspace_bytes, &workspace));
        CHECK_STATUS(seen_cuda_allocation_address(workspace, &workspace_ptr,
            &ignored_bytes, &ignored_device));
    }
    CHECK_STATUS(seen_cublaslt_matmul(lt, &desc, &algorithm, a_ptr, b_ptr,
        c_ptr, workspace_ptr, stream));
    CHECK_STATUS(seen_cuda_memcpy_async(host_c.data(), c_ptr, bytes,
        SEEN_CUDA_COPY_DEVICE_TO_HOST, stream));
    CHECK_STATUS(seen_cuda_stream_synchronize(stream));
    for (T value : host_c) CHECK(std::fabs(as_float(value) - 16.0f) <= 0.02f);
    CHECK_STATUS(seen_cuda_free(&workspace));
    CHECK_STATUS(seen_cuda_free(&c)); CHECK_STATUS(seen_cuda_free(&b));
    CHECK_STATUS(seen_cuda_free(&a));
    return 0;
}

int main() {
    CHECK(seen_cuda_abi_version() == SEEN_CUDA_ABI_VERSION);
    int32_t count = 0;
    CHECK_STATUS(seen_cuda_device_count(&count));
    CHECK(count > 0);
    SeenCudaDeviceInfo info{};
    CHECK_STATUS(seen_cuda_device_get(0, &info));
    CHECK(info.compute_major == 8 && info.compute_minor == 9);
    CHECK(info.total_memory_bytes >= UINT64_C(20) * 1024 * 1024 * 1024);
    int32_t compute_major = 0, compute_minor = 0;
    uint64_t total_memory = 0, free_memory = 0;
    CHECK_STATUS(seen_cuda_device_capability(0, &compute_major, &compute_minor,
        &total_memory, &free_memory));
    CHECK(compute_major == info.compute_major && compute_minor == info.compute_minor);
    CHECK(total_memory == info.total_memory_bytes && free_memory <= total_memory);
    SeenCudaHandle rejected = 0;
    SeenCudaStatus invalid_allocation = seen_cuda_malloc(0, 0, &rejected);
    CHECK(invalid_allocation.code == SEEN_CUDA_INVALID_ARGUMENT && rejected == 0);
    if (std::getenv("SEEN_QWEN_SKIP_OOM_PROBE") == nullptr) {
        SeenCudaStatus bounded_oom = seen_cuda_malloc(0, UINT64_MAX, &rejected);
        CHECK(bounded_oom.code == SEEN_CUDA_OUT_OF_MEMORY ||
              bounded_oom.code == SEEN_CUDA_RUNTIME_ERROR ||
              bounded_oom.code == SEEN_CUDA_LIMIT);
        CHECK(rejected == 0);
    }

    SeenCudaHandle stream = 0, start = 0, end = 0, allocation = 0, host = 0;
    CHECK_STATUS(seen_cuda_stream_create(0, &stream));
    CHECK_STATUS(seen_cuda_event_create(0, &start));
    CHECK_STATUS(seen_cuda_event_create(0, &end));
    CHECK_STATUS(seen_cuda_malloc(0, 4096, &allocation));
    CHECK_STATUS(seen_cuda_host_alloc(4096, &host));
    void *device_data = nullptr, *host_data = nullptr;
    uint64_t bytes = 0; int32_t device = -1;
    CHECK_STATUS(seen_cuda_allocation_address(allocation, &device_data, &bytes, &device));
    CHECK_STATUS(seen_cuda_host_allocation_address(host, &host_data, &bytes));
    CHECK_STATUS(seen_cuda_event_record(start, stream));
    CHECK_STATUS(seen_cuda_graph_begin_capture(stream));
    CHECK_STATUS(seen_cuda_memset_async(device_data, 0x2a, 4096, stream));
    SeenCudaHandle graph = 0, graph_exec = 0;
    CHECK_STATUS(seen_cuda_graph_end_capture(stream, &graph));
    CHECK_STATUS(seen_cuda_graph_instantiate(graph, &graph_exec));
    CHECK_STATUS(seen_cuda_graph_launch(graph_exec, stream));
    CHECK_STATUS(seen_cuda_memcpy_async(host_data, device_data, 4096,
        SEEN_CUDA_COPY_DEVICE_TO_HOST, stream));
    CHECK_STATUS(seen_cuda_event_record(end, stream));
    CHECK_STATUS(seen_cuda_event_synchronize(end));
    float elapsed = 0.0f;
    CHECK_STATUS(seen_cuda_event_elapsed_ms(start, end, &elapsed));
    for (size_t index = 0; index < 4096; ++index)
        CHECK(static_cast<unsigned char *>(host_data)[index] == 0x2a);
    int32_t updated = 0;
    CHECK_STATUS(seen_cuda_graph_exec_update(graph_exec, graph, &updated));
    CHECK(updated == 1);

    SeenCudaHandle lt = 0;
    CHECK_STATUS(seen_cublaslt_create(0, &lt));
    CHECK(matmul_case<__half>(lt, stream, SEEN_CUDA_F16) == 0);
    CHECK(matmul_case<__nv_bfloat16>(lt, stream, SEEN_CUDA_BF16) == 0);
    CHECK_STATUS(seen_cublaslt_destroy(&lt));
    CHECK_STATUS(seen_cublaslt_destroy(&lt));
    CHECK_STATUS(seen_cuda_graph_exec_destroy(&graph_exec));
    CHECK_STATUS(seen_cuda_graph_exec_destroy(&graph_exec));
    CHECK_STATUS(seen_cuda_graph_destroy(&graph));
    CHECK_STATUS(seen_cuda_graph_destroy(&graph));
    CHECK_STATUS(seen_cuda_host_free(&host));
    CHECK_STATUS(seen_cuda_host_free(&host));
    CHECK_STATUS(seen_cuda_free(&allocation));
    CHECK_STATUS(seen_cuda_free(&allocation));
    CHECK_STATUS(seen_cuda_event_destroy(&end));
    CHECK_STATUS(seen_cuda_event_destroy(&end));
    CHECK_STATUS(seen_cuda_event_destroy(&start));
    CHECK_STATUS(seen_cuda_event_destroy(&start));
    CHECK_STATUS(seen_cuda_stream_destroy(&stream));
    CHECK_STATUS(seen_cuda_stream_destroy(&stream));
    std::printf("PASS: RTX 4090 CUDA memory, stream, event, graph, F16 and BF16 cuBLASLt\n");
    return 0;
}
