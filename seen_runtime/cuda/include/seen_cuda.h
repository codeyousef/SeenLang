#ifndef SEEN_CUDA_H
#define SEEN_CUDA_H

#include <stddef.h>
#include <stdint.h>

#if defined(_WIN32)
#define SEEN_CUDA_EXPORT __declspec(dllexport)
#else
#define SEEN_CUDA_EXPORT __attribute__((visibility("default")))
#endif

#ifdef __cplusplus
extern "C" {
#endif

#define SEEN_CUDA_ABI_VERSION 1u
typedef uint64_t SeenCudaHandle;

typedef enum SeenCudaCode {
    SEEN_CUDA_OK = 0,
    SEEN_CUDA_INVALID_ARGUMENT = 1,
    SEEN_CUDA_UNSUPPORTED = 2,
    SEEN_CUDA_NOT_FOUND = 3,
    SEEN_CUDA_OUT_OF_MEMORY = 4,
    SEEN_CUDA_RUNTIME_ERROR = 5,
    SEEN_CUDA_CUBLAS_ERROR = 6,
    SEEN_CUDA_BUSY = 7,
    SEEN_CUDA_CLOSED = 8,
    SEEN_CUDA_LIMIT = 9,
    SEEN_CUDA_INCOMPATIBLE = 10
} SeenCudaCode;

typedef enum SeenCudaMaturity {
    SEEN_CUDA_MATURITY_UNSUPPORTED = 0,
    SEEN_CUDA_MATURITY_COMPILE_ONLY = 1,
    SEEN_CUDA_MATURITY_EXPERIMENTAL_HARDWARE = 2,
    SEEN_CUDA_MATURITY_VERIFIED = 3,
    SEEN_CUDA_MATURITY_PRODUCTION_CERTIFIED = 4
} SeenCudaMaturity;

typedef struct SeenCudaStatus {
    int32_t code;
    int32_t native_code;
    int32_t device_ordinal;
    int32_t maturity;
    const char *operation;
    const char *message;
} SeenCudaStatus;

typedef struct SeenCudaDeviceInfo {
    int32_t ordinal;
    int32_t compute_major;
    int32_t compute_minor;
    int32_t multiprocessors;
    uint64_t total_memory_bytes;
    uint64_t free_memory_bytes;
    char name[128];
    char pci_bus_id[32];
    char uuid[48];
} SeenCudaDeviceInfo;

typedef enum SeenCudaMemcpyKind {
    SEEN_CUDA_COPY_HOST_TO_DEVICE = 1,
    SEEN_CUDA_COPY_DEVICE_TO_HOST = 2,
    SEEN_CUDA_COPY_DEVICE_TO_DEVICE = 3
} SeenCudaMemcpyKind;

typedef enum SeenCudaDataType {
    SEEN_CUDA_F16 = 1,
    SEEN_CUDA_BF16 = 2,
    SEEN_CUDA_F32 = 3
} SeenCudaDataType;

typedef struct SeenCudaMatmulDesc {
    uint32_t abi_version;
    int32_t data_type;
    int32_t transpose_a;
    int32_t transpose_b;
    uint64_t m;
    uint64_t n;
    uint64_t k;
    int64_t leading_a;
    int64_t leading_b;
    int64_t leading_c;
    uint64_t workspace_limit_bytes;
} SeenCudaMatmulDesc;

typedef struct SeenCudaAlgorithm {
    int32_t algorithm_id;
    int32_t tile_id;
    int32_t split_k;
    int32_t reduction_scheme;
    uint64_t workspace_bytes;
    uint64_t cache_identity;
} SeenCudaAlgorithm;

SEEN_CUDA_EXPORT uint32_t seen_cuda_abi_version(void);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_runtime_version(int32_t *version);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_driver_version(int32_t *version);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_device_count(int32_t *count);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_device_get(int32_t ordinal,
                                                      SeenCudaDeviceInfo *info);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_device_capability(int32_t ordinal,
    int32_t *compute_major, int32_t *compute_minor,
    uint64_t *total_memory_bytes, uint64_t *free_memory_bytes);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_set_device(int32_t ordinal);

SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_malloc(int32_t device,
    uint64_t bytes, SeenCudaHandle *allocation);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_free(SeenCudaHandle *allocation);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_allocation_address(
    SeenCudaHandle allocation, void **address, uint64_t *bytes, int32_t *device);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_host_alloc(uint64_t bytes,
    SeenCudaHandle *allocation);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_host_free(SeenCudaHandle *allocation);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_host_allocation_address(
    SeenCudaHandle allocation, void **address, uint64_t *bytes);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_memcpy_async(void *destination,
    const void *source, uint64_t bytes, int32_t kind, SeenCudaHandle stream);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_memset_async(void *destination,
    int32_t value, uint64_t bytes, SeenCudaHandle stream);

SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_stream_create(int32_t device,
    SeenCudaHandle *stream);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_stream_destroy(SeenCudaHandle *stream);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_stream_synchronize(SeenCudaHandle stream);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_event_create(int32_t device,
    SeenCudaHandle *event);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_event_record(SeenCudaHandle event,
    SeenCudaHandle stream);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_event_synchronize(SeenCudaHandle event);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_event_elapsed_ms(SeenCudaHandle start,
    SeenCudaHandle end, float *milliseconds);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_event_destroy(SeenCudaHandle *event);

SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_graph_begin_capture(SeenCudaHandle stream);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_graph_end_capture(SeenCudaHandle stream,
    SeenCudaHandle *graph);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_graph_instantiate(SeenCudaHandle graph,
    SeenCudaHandle *graph_exec);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_graph_launch(SeenCudaHandle graph_exec,
    SeenCudaHandle stream);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_graph_exec_update(
    SeenCudaHandle graph_exec, SeenCudaHandle graph, int32_t *updated);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_graph_exec_destroy(
    SeenCudaHandle *graph_exec);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cuda_graph_destroy(SeenCudaHandle *graph);

SEEN_CUDA_EXPORT SeenCudaStatus seen_cublaslt_create(int32_t device,
    SeenCudaHandle *handle);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cublaslt_destroy(SeenCudaHandle *handle);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cublaslt_select_algorithm(
    SeenCudaHandle handle, const SeenCudaMatmulDesc *descriptor,
    SeenCudaAlgorithm *algorithm);
SEEN_CUDA_EXPORT SeenCudaStatus seen_cublaslt_matmul(SeenCudaHandle handle,
    const SeenCudaMatmulDesc *descriptor, const SeenCudaAlgorithm *algorithm,
    const void *a, const void *b, void *c, void *workspace,
    SeenCudaHandle stream);

#ifdef __cplusplus
}
#endif
#endif
