#include "seen_cuda.h"

#include <cublasLt.h>
#include <cuda_runtime_api.h>

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <limits>

namespace {

constexpr uint64_t kAllocationMagic = UINT64_C(0x5343414c4c4f4331);
constexpr uint64_t kHostMagic = UINT64_C(0x5343484f53543031);
constexpr uint64_t kStreamMagic = UINT64_C(0x534353545245414d);
constexpr uint64_t kEventMagic = UINT64_C(0x53434556454e5431);
constexpr uint64_t kGraphMagic = UINT64_C(0x5343475241504831);
constexpr uint64_t kGraphExecMagic = UINT64_C(0x5343474558454331);
constexpr uint64_t kLtMagic = UINT64_C(0x53434355424c5431);

struct Allocation { uint64_t magic; void *data; uint64_t bytes; int device; };
struct HostAllocation { uint64_t magic; void *data; uint64_t bytes; };
struct Stream { uint64_t magic; cudaStream_t value; int device; bool capturing; };
struct Event { uint64_t magic; cudaEvent_t value; int device; };
struct Graph { uint64_t magic; cudaGraph_t value; int device; };
struct GraphExec { uint64_t magic; cudaGraphExec_t value; int device; };
struct LtHandle { uint64_t magic; cublasLtHandle_t value; int device; };

SeenCudaStatus status(int code, int native, int device, const char *operation,
                      const char *message) {
    SeenCudaStatus result{};
    result.code = code;
    result.native_code = native;
    result.device_ordinal = device;
    result.maturity = SEEN_CUDA_MATURITY_EXPERIMENTAL_HARDWARE;
    result.operation = operation ? operation : "unknown";
    result.message = message ? message : "";
    return result;
}

SeenCudaStatus ok(const char *operation, int device = -1) {
    return status(SEEN_CUDA_OK, 0, device, operation, "ok");
}

SeenCudaStatus invalid(const char *operation, const char *message) {
    return status(SEEN_CUDA_INVALID_ARGUMENT, 0, -1, operation, message);
}

SeenCudaStatus cuda_failure(const char *operation, cudaError_t error,
                            int device = -1) {
    const int code = error == cudaErrorMemoryAllocation
        ? SEEN_CUDA_OUT_OF_MEMORY : SEEN_CUDA_RUNTIME_ERROR;
    return status(code, static_cast<int>(error), device, operation,
                  cudaGetErrorString(error));
}

SeenCudaStatus cublas_failure(const char *operation, cublasStatus_t error,
                              int device = -1) {
    return status(SEEN_CUDA_CUBLAS_ERROR, static_cast<int>(error), device,
                  operation, "cuBLASLt operation failed");
}

template <typename T>
T *checked(SeenCudaHandle handle, uint64_t magic) {
    auto *value = reinterpret_cast<T *>(static_cast<uintptr_t>(handle));
    return value && value->magic == magic ? value : nullptr;
}

SeenCudaStatus select_device(int device, const char *operation) {
    if (device < 0) return invalid(operation, "negative CUDA device ordinal");
    cudaError_t error = cudaSetDevice(device);
    return error == cudaSuccess ? ok(operation, device)
                                : cuda_failure(operation, error, device);
}

uint64_t fnv1a(const void *data, size_t size, uint64_t hash) {
    const auto *bytes = static_cast<const uint8_t *>(data);
    for (size_t index = 0; index < size; ++index) {
        hash ^= bytes[index];
        hash *= UINT64_C(1099511628211);
    }
    return hash;
}

cudaDataType_t cuda_type(int type) {
    if (type == SEEN_CUDA_F16) return CUDA_R_16F;
    if (type == SEEN_CUDA_BF16) return CUDA_R_16BF;
    return CUDA_R_32F;
}

struct LtDescriptors {
    cublasLtMatmulDesc_t operation = nullptr;
    cublasLtMatrixLayout_t a = nullptr;
    cublasLtMatrixLayout_t b = nullptr;
    cublasLtMatrixLayout_t c = nullptr;
    cublasLtMatmulPreference_t preference = nullptr;
};

void destroy_descriptors(LtDescriptors *d) {
    if (d->preference) cublasLtMatmulPreferenceDestroy(d->preference);
    if (d->c) cublasLtMatrixLayoutDestroy(d->c);
    if (d->b) cublasLtMatrixLayoutDestroy(d->b);
    if (d->a) cublasLtMatrixLayoutDestroy(d->a);
    if (d->operation) cublasLtMatmulDescDestroy(d->operation);
    *d = LtDescriptors{};
}

cublasStatus_t create_descriptors(const SeenCudaMatmulDesc *desc,
                                  LtDescriptors *out) {
    const cudaDataType_t type = cuda_type(desc->data_type);
    cublasStatus_t result = cublasLtMatmulDescCreate(
        &out->operation, CUBLAS_COMPUTE_32F, CUDA_R_32F);
    if (result != CUBLAS_STATUS_SUCCESS) return result;
    cublasOperation_t trans_a = desc->transpose_a ? CUBLAS_OP_T : CUBLAS_OP_N;
    cublasOperation_t trans_b = desc->transpose_b ? CUBLAS_OP_T : CUBLAS_OP_N;
    result = cublasLtMatmulDescSetAttribute(out->operation,
        CUBLASLT_MATMUL_DESC_TRANSA, &trans_a, sizeof(trans_a));
    if (result != CUBLAS_STATUS_SUCCESS) return result;
    result = cublasLtMatmulDescSetAttribute(out->operation,
        CUBLASLT_MATMUL_DESC_TRANSB, &trans_b, sizeof(trans_b));
    if (result != CUBLAS_STATUS_SUCCESS) return result;
    const uint64_t a_rows = desc->transpose_a ? desc->k : desc->m;
    const uint64_t a_cols = desc->transpose_a ? desc->m : desc->k;
    const uint64_t b_rows = desc->transpose_b ? desc->n : desc->k;
    const uint64_t b_cols = desc->transpose_b ? desc->k : desc->n;
    result = cublasLtMatrixLayoutCreate(&out->a, type, a_rows, a_cols,
                                        desc->leading_a);
    if (result != CUBLAS_STATUS_SUCCESS) return result;
    result = cublasLtMatrixLayoutCreate(&out->b, type, b_rows, b_cols,
                                        desc->leading_b);
    if (result != CUBLAS_STATUS_SUCCESS) return result;
    result = cublasLtMatrixLayoutCreate(&out->c, type, desc->m, desc->n,
                                        desc->leading_c);
    if (result != CUBLAS_STATUS_SUCCESS) return result;
    result = cublasLtMatmulPreferenceCreate(&out->preference);
    if (result != CUBLAS_STATUS_SUCCESS) return result;
    return cublasLtMatmulPreferenceSetAttribute(out->preference,
        CUBLASLT_MATMUL_PREF_MAX_WORKSPACE_BYTES,
        &desc->workspace_limit_bytes, sizeof(desc->workspace_limit_bytes));
}

SeenCudaStatus validate_matmul(const SeenCudaMatmulDesc *desc,
                               const char *operation) {
    if (!desc || desc->abi_version != SEEN_CUDA_ABI_VERSION ||
        (desc->data_type != SEEN_CUDA_F16 &&
         desc->data_type != SEEN_CUDA_BF16) ||
        desc->m == 0 || desc->n == 0 || desc->k == 0 ||
        desc->m > static_cast<uint64_t>(std::numeric_limits<int32_t>::max()) ||
        desc->n > static_cast<uint64_t>(std::numeric_limits<int32_t>::max()) ||
        desc->k > static_cast<uint64_t>(std::numeric_limits<int32_t>::max()) ||
        desc->leading_a <= 0 || desc->leading_b <= 0 || desc->leading_c <= 0) {
        return invalid(operation, "invalid or unsupported bounded matmul descriptor");
    }
    return ok(operation);
}

SeenCudaStatus select_algorithm_internal(LtHandle *handle,
    const SeenCudaMatmulDesc *descriptor, SeenCudaAlgorithm *algorithm,
    cublasLtMatmulHeuristicResult_t *selected) {
    SeenCudaStatus valid = validate_matmul(descriptor, "cublaslt-select-algorithm");
    if (valid.code != SEEN_CUDA_OK || !algorithm)
        return valid.code == SEEN_CUDA_OK
            ? invalid("cublaslt-select-algorithm", "missing algorithm output")
            : valid;
    LtDescriptors d;
    cublasStatus_t cstatus = create_descriptors(descriptor, &d);
    if (cstatus != CUBLAS_STATUS_SUCCESS) {
        destroy_descriptors(&d);
        return cublas_failure("cublaslt-select-algorithm", cstatus, handle->device);
    }
    cublasLtMatmulHeuristicResult_t candidates[32]{};
    int returned = 0;
    cstatus = cublasLtMatmulAlgoGetHeuristic(handle->value, d.operation,
        d.a, d.b, d.c, d.c, d.preference, 32, candidates, &returned);
    destroy_descriptors(&d);
    if (cstatus != CUBLAS_STATUS_SUCCESS)
        return cublas_failure("cublaslt-select-algorithm", cstatus, handle->device);
    int chosen = -1;
    int best_id = std::numeric_limits<int>::max();
    for (int index = 0; index < returned; ++index) {
        if (candidates[index].state != CUBLAS_STATUS_SUCCESS ||
            candidates[index].workspaceSize > descriptor->workspace_limit_bytes)
            continue;
        int id = 0;
        size_t written = 0;
        cublasLtMatmulAlgoConfigGetAttribute(&candidates[index].algo,
            CUBLASLT_ALGO_CONFIG_ID, &id, sizeof(id), &written);
        if (written == sizeof(id) && id < best_id) {
            best_id = id;
            chosen = index;
        }
    }
    if (chosen < 0)
        return status(SEEN_CUDA_NOT_FOUND, 0, handle->device,
            "cublaslt-select-algorithm", "no algorithm fits the workspace bound");
    int tile = 0, split_k = 0, reduction = 0;
    size_t ignored = 0;
    cublasLtMatmulAlgoConfigGetAttribute(&candidates[chosen].algo,
        CUBLASLT_ALGO_CONFIG_TILE_ID, &tile, sizeof(tile), &ignored);
    cublasLtMatmulAlgoConfigGetAttribute(&candidates[chosen].algo,
        CUBLASLT_ALGO_CONFIG_SPLITK_NUM, &split_k, sizeof(split_k), &ignored);
    cublasLtMatmulAlgoConfigGetAttribute(&candidates[chosen].algo,
        CUBLASLT_ALGO_CONFIG_REDUCTION_SCHEME, &reduction,
        sizeof(reduction), &ignored);
    algorithm->algorithm_id = best_id;
    algorithm->tile_id = tile;
    algorithm->split_k = split_k;
    algorithm->reduction_scheme = reduction;
    algorithm->workspace_bytes = candidates[chosen].workspaceSize;
    uint64_t hash = UINT64_C(1469598103934665603);
    hash = fnv1a(descriptor, sizeof(*descriptor), hash);
    hash = fnv1a(algorithm, offsetof(SeenCudaAlgorithm, cache_identity), hash);
    algorithm->cache_identity = hash;
    if (selected) *selected = candidates[chosen];
    return ok("cublaslt-select-algorithm", handle->device);
}

}  // namespace

extern "C" {

uint32_t seen_cuda_abi_version(void) { return SEEN_CUDA_ABI_VERSION; }

SeenCudaStatus seen_cuda_runtime_version(int32_t *version) {
    if (!version) return invalid("runtime-version", "missing output");
    int value = 0;
    cudaError_t error = cudaRuntimeGetVersion(&value);
    if (error != cudaSuccess) return cuda_failure("runtime-version", error);
    *version = value;
    return ok("runtime-version");
}

SeenCudaStatus seen_cuda_driver_version(int32_t *version) {
    if (!version) return invalid("driver-version", "missing output");
    int value = 0;
    cudaError_t error = cudaDriverGetVersion(&value);
    if (error != cudaSuccess) return cuda_failure("driver-version", error);
    *version = value;
    return ok("driver-version");
}

SeenCudaStatus seen_cuda_device_count(int32_t *count) {
    if (!count) return invalid("device-count", "missing output");
    int value = 0;
    cudaError_t error = cudaGetDeviceCount(&value);
    if (error != cudaSuccess) return cuda_failure("device-count", error);
    *count = value;
    return ok("device-count");
}

SeenCudaStatus seen_cuda_device_get(int32_t ordinal, SeenCudaDeviceInfo *info) {
    if (!info || ordinal < 0) return invalid("device-get", "invalid device query");
    cudaDeviceProp property{};
    cudaError_t error = cudaGetDeviceProperties(&property, ordinal);
    if (error != cudaSuccess) return cuda_failure("device-get", error, ordinal);
    size_t free_bytes = 0, total_bytes = 0;
    SeenCudaStatus selected = select_device(ordinal, "device-get");
    if (selected.code != SEEN_CUDA_OK) return selected;
    error = cudaMemGetInfo(&free_bytes, &total_bytes);
    if (error != cudaSuccess) return cuda_failure("device-get", error, ordinal);
    *info = SeenCudaDeviceInfo{};
    info->ordinal = ordinal;
    info->compute_major = property.major;
    info->compute_minor = property.minor;
    info->multiprocessors = property.multiProcessorCount;
    info->total_memory_bytes = static_cast<uint64_t>(total_bytes);
    info->free_memory_bytes = static_cast<uint64_t>(free_bytes);
    std::snprintf(info->name, sizeof(info->name), "%s", property.name);
    cudaDeviceGetPCIBusId(info->pci_bus_id, sizeof(info->pci_bus_id), ordinal);
    char *uuid = info->uuid;
    size_t remaining = sizeof(info->uuid);
    for (size_t index = 0; index < sizeof(property.uuid.bytes) && remaining > 2; ++index) {
        int used = std::snprintf(uuid, remaining, "%02x",
            static_cast<unsigned char>(property.uuid.bytes[index]));
        uuid += used;
        remaining -= static_cast<size_t>(used);
    }
    return ok("device-get", ordinal);
}

SeenCudaStatus seen_cuda_device_capability(int32_t ordinal,
    int32_t *compute_major, int32_t *compute_minor,
    uint64_t *total_memory_bytes, uint64_t *free_memory_bytes) {
    if (!compute_major || !compute_minor || !total_memory_bytes ||
        !free_memory_bytes)
        return invalid("device-capability", "missing device capability output");
    SeenCudaDeviceInfo info{};
    SeenCudaStatus queried = seen_cuda_device_get(ordinal, &info);
    if (queried.code != SEEN_CUDA_OK) return queried;
    *compute_major = info.compute_major;
    *compute_minor = info.compute_minor;
    *total_memory_bytes = info.total_memory_bytes;
    *free_memory_bytes = info.free_memory_bytes;
    return ok("device-capability", ordinal);
}

SeenCudaStatus seen_cuda_set_device(int32_t ordinal) {
    return select_device(ordinal, "set-device");
}

SeenCudaStatus seen_cuda_malloc(int32_t device, uint64_t bytes,
                                SeenCudaHandle *allocation) {
    if (!allocation || bytes == 0 || bytes > SIZE_MAX)
        return invalid("malloc", "invalid bounded allocation request");
    *allocation = 0;
    SeenCudaStatus selected = select_device(device, "malloc");
    if (selected.code != SEEN_CUDA_OK) return selected;
    auto *object = static_cast<Allocation *>(std::calloc(1, sizeof(Allocation)));
    if (!object) return status(SEEN_CUDA_OUT_OF_MEMORY, 0, device, "malloc",
                               "host allocation metadata failed");
    cudaError_t error = cudaMalloc(&object->data, static_cast<size_t>(bytes));
    if (error != cudaSuccess) { std::free(object); return cuda_failure("malloc", error, device); }
    object->magic = kAllocationMagic; object->bytes = bytes; object->device = device;
    *allocation = reinterpret_cast<uintptr_t>(object);
    return ok("malloc", device);
}

SeenCudaStatus seen_cuda_free(SeenCudaHandle *allocation) {
    if (!allocation) return invalid("free", "missing owner handle");
    if (*allocation == 0) return ok("free");
    Allocation *object = checked<Allocation>(*allocation, kAllocationMagic);
    if (!object) return invalid("free", "invalid allocation owner");
    SeenCudaStatus selected = select_device(object->device, "free");
    if (selected.code != SEEN_CUDA_OK) return selected;
    cudaError_t error = cudaFree(object->data);
    if (error != cudaSuccess) return cuda_failure("free", error, object->device);
    object->magic = 0; std::free(object); *allocation = 0;
    return ok("free");
}

SeenCudaStatus seen_cuda_allocation_address(SeenCudaHandle allocation,
    void **address, uint64_t *bytes, int32_t *device) {
    Allocation *object = checked<Allocation>(allocation, kAllocationMagic);
    if (!object || !address || !bytes || !device)
        return invalid("allocation-address", "invalid allocation query");
    *address = object->data; *bytes = object->bytes; *device = object->device;
    return ok("allocation-address", object->device);
}

SeenCudaStatus seen_cuda_host_alloc(uint64_t bytes, SeenCudaHandle *allocation) {
    if (!allocation || bytes == 0 || bytes > SIZE_MAX)
        return invalid("host-alloc", "invalid bounded host allocation request");
    *allocation = 0;
    auto *object = static_cast<HostAllocation *>(std::calloc(1, sizeof(HostAllocation)));
    if (!object) return status(SEEN_CUDA_OUT_OF_MEMORY, 0, -1, "host-alloc",
                               "host allocation metadata failed");
    cudaError_t error = cudaHostAlloc(&object->data, static_cast<size_t>(bytes),
                                      cudaHostAllocPortable);
    if (error != cudaSuccess) { std::free(object); return cuda_failure("host-alloc", error); }
    object->magic = kHostMagic; object->bytes = bytes;
    *allocation = reinterpret_cast<uintptr_t>(object);
    return ok("host-alloc");
}

SeenCudaStatus seen_cuda_host_free(SeenCudaHandle *allocation) {
    if (!allocation) return invalid("host-free", "missing owner handle");
    if (*allocation == 0) return ok("host-free");
    HostAllocation *object = checked<HostAllocation>(*allocation, kHostMagic);
    if (!object) return invalid("host-free", "invalid host allocation owner");
    cudaError_t error = cudaFreeHost(object->data);
    if (error != cudaSuccess) return cuda_failure("host-free", error);
    object->magic = 0; std::free(object); *allocation = 0;
    return ok("host-free");
}

SeenCudaStatus seen_cuda_host_allocation_address(SeenCudaHandle allocation,
    void **address, uint64_t *bytes) {
    HostAllocation *object = checked<HostAllocation>(allocation, kHostMagic);
    if (!object || !address || !bytes)
        return invalid("host-allocation-address", "invalid host allocation query");
    *address = object->data; *bytes = object->bytes;
    return ok("host-allocation-address");
}

SeenCudaStatus seen_cuda_stream_create(int32_t device, SeenCudaHandle *stream) {
    if (!stream) return invalid("stream-create", "missing output");
    *stream = 0;
    SeenCudaStatus selected = select_device(device, "stream-create");
    if (selected.code != SEEN_CUDA_OK) return selected;
    auto *object = static_cast<Stream *>(std::calloc(1, sizeof(Stream)));
    if (!object) return status(SEEN_CUDA_OUT_OF_MEMORY, 0, device,
                               "stream-create", "stream metadata allocation failed");
    cudaError_t error = cudaStreamCreateWithFlags(&object->value, cudaStreamNonBlocking);
    if (error != cudaSuccess) { std::free(object); return cuda_failure("stream-create", error, device); }
    object->magic = kStreamMagic; object->device = device;
    *stream = reinterpret_cast<uintptr_t>(object);
    return ok("stream-create", device);
}

SeenCudaStatus seen_cuda_stream_destroy(SeenCudaHandle *stream) {
    if (!stream) return invalid("stream-destroy", "missing owner handle");
    if (*stream == 0) return ok("stream-destroy");
    Stream *object = checked<Stream>(*stream, kStreamMagic);
    if (!object || object->capturing)
        return status(object ? SEEN_CUDA_BUSY : SEEN_CUDA_INVALID_ARGUMENT,
            0, object ? object->device : -1, "stream-destroy",
            object ? "stream capture is active" : "invalid stream owner");
    cudaError_t error = cudaStreamDestroy(object->value);
    if (error != cudaSuccess) return cuda_failure("stream-destroy", error, object->device);
    object->magic = 0; std::free(object); *stream = 0;
    return ok("stream-destroy");
}

SeenCudaStatus seen_cuda_stream_synchronize(SeenCudaHandle stream) {
    Stream *object = checked<Stream>(stream, kStreamMagic);
    if (!object) return invalid("stream-synchronize", "invalid stream");
    cudaError_t error = cudaStreamSynchronize(object->value);
    return error == cudaSuccess ? ok("stream-synchronize", object->device)
                                : cuda_failure("stream-synchronize", error, object->device);
}

SeenCudaStatus seen_cuda_memcpy_async(void *destination, const void *source,
    uint64_t bytes, int32_t kind, SeenCudaHandle stream) {
    Stream *object = checked<Stream>(stream, kStreamMagic);
    if (!object || !destination || !source || bytes == 0 || bytes > SIZE_MAX ||
        kind < SEEN_CUDA_COPY_HOST_TO_DEVICE || kind > SEEN_CUDA_COPY_DEVICE_TO_DEVICE)
        return invalid("memcpy-async", "invalid bounded asynchronous copy");
    cudaMemcpyKind native_kind = kind == SEEN_CUDA_COPY_HOST_TO_DEVICE
        ? cudaMemcpyHostToDevice : kind == SEEN_CUDA_COPY_DEVICE_TO_HOST
        ? cudaMemcpyDeviceToHost : cudaMemcpyDeviceToDevice;
    cudaError_t error = cudaMemcpyAsync(destination, source, static_cast<size_t>(bytes),
                                        native_kind, object->value);
    return error == cudaSuccess ? ok("memcpy-async", object->device)
                                : cuda_failure("memcpy-async", error, object->device);
}

SeenCudaStatus seen_cuda_memset_async(void *destination, int32_t value,
    uint64_t bytes, SeenCudaHandle stream) {
    Stream *object = checked<Stream>(stream, kStreamMagic);
    if (!object || !destination || bytes == 0 || bytes > SIZE_MAX)
        return invalid("memset-async", "invalid bounded asynchronous fill");
    cudaError_t error = cudaMemsetAsync(destination, value, static_cast<size_t>(bytes),
                                        object->value);
    return error == cudaSuccess ? ok("memset-async", object->device)
                                : cuda_failure("memset-async", error, object->device);
}

SeenCudaStatus seen_cuda_event_create(int32_t device, SeenCudaHandle *event) {
    if (!event) return invalid("event-create", "missing output");
    *event = 0;
    SeenCudaStatus selected = select_device(device, "event-create");
    if (selected.code != SEEN_CUDA_OK) return selected;
    auto *object = static_cast<Event *>(std::calloc(1, sizeof(Event)));
    if (!object) return status(SEEN_CUDA_OUT_OF_MEMORY, 0, device,
                               "event-create", "event metadata allocation failed");
    cudaError_t error = cudaEventCreate(&object->value);
    if (error != cudaSuccess) { std::free(object); return cuda_failure("event-create", error, device); }
    object->magic = kEventMagic; object->device = device;
    *event = reinterpret_cast<uintptr_t>(object);
    return ok("event-create", device);
}

SeenCudaStatus seen_cuda_event_record(SeenCudaHandle event, SeenCudaHandle stream) {
    Event *event_object = checked<Event>(event, kEventMagic);
    Stream *stream_object = checked<Stream>(stream, kStreamMagic);
    if (!event_object || !stream_object || event_object->device != stream_object->device)
        return invalid("event-record", "invalid or cross-device event/stream");
    cudaError_t error = cudaEventRecord(event_object->value, stream_object->value);
    return error == cudaSuccess ? ok("event-record", event_object->device)
                                : cuda_failure("event-record", error, event_object->device);
}

SeenCudaStatus seen_cuda_event_synchronize(SeenCudaHandle event) {
    Event *object = checked<Event>(event, kEventMagic);
    if (!object) return invalid("event-synchronize", "invalid event");
    cudaError_t error = cudaEventSynchronize(object->value);
    return error == cudaSuccess ? ok("event-synchronize", object->device)
                                : cuda_failure("event-synchronize", error, object->device);
}

SeenCudaStatus seen_cuda_event_elapsed_ms(SeenCudaHandle start,
    SeenCudaHandle end, float *milliseconds) {
    Event *start_object = checked<Event>(start, kEventMagic);
    Event *end_object = checked<Event>(end, kEventMagic);
    if (!start_object || !end_object || !milliseconds ||
        start_object->device != end_object->device)
        return invalid("event-elapsed", "invalid or cross-device event pair");
    cudaError_t error = cudaEventElapsedTime(milliseconds, start_object->value,
                                             end_object->value);
    return error == cudaSuccess ? ok("event-elapsed", start_object->device)
                                : cuda_failure("event-elapsed", error, start_object->device);
}

SeenCudaStatus seen_cuda_event_destroy(SeenCudaHandle *event) {
    if (!event) return invalid("event-destroy", "missing owner handle");
    if (*event == 0) return ok("event-destroy");
    Event *object = checked<Event>(*event, kEventMagic);
    if (!object) return invalid("event-destroy", "invalid event owner");
    cudaError_t error = cudaEventDestroy(object->value);
    if (error != cudaSuccess) return cuda_failure("event-destroy", error, object->device);
    object->magic = 0; std::free(object); *event = 0;
    return ok("event-destroy");
}

SeenCudaStatus seen_cuda_graph_begin_capture(SeenCudaHandle stream) {
    Stream *object = checked<Stream>(stream, kStreamMagic);
    if (!object || object->capturing)
        return invalid("graph-begin-capture", "invalid stream or capture already active");
    cudaError_t error = cudaStreamBeginCapture(object->value, cudaStreamCaptureModeThreadLocal);
    if (error != cudaSuccess) return cuda_failure("graph-begin-capture", error, object->device);
    object->capturing = true;
    return ok("graph-begin-capture", object->device);
}

SeenCudaStatus seen_cuda_graph_end_capture(SeenCudaHandle stream,
                                           SeenCudaHandle *graph) {
    Stream *stream_object = checked<Stream>(stream, kStreamMagic);
    if (!stream_object || !stream_object->capturing || !graph)
        return invalid("graph-end-capture", "capture is not active or output is missing");
    *graph = 0;
    cudaGraph_t native_graph = nullptr;
    cudaError_t error = cudaStreamEndCapture(stream_object->value, &native_graph);
    stream_object->capturing = false;
    if (error != cudaSuccess) return cuda_failure("graph-end-capture", error, stream_object->device);
    auto *object = static_cast<Graph *>(std::calloc(1, sizeof(Graph)));
    if (!object) { cudaGraphDestroy(native_graph); return status(SEEN_CUDA_OUT_OF_MEMORY,
        0, stream_object->device, "graph-end-capture", "graph metadata allocation failed"); }
    object->magic = kGraphMagic; object->value = native_graph; object->device = stream_object->device;
    *graph = reinterpret_cast<uintptr_t>(object);
    return ok("graph-end-capture", object->device);
}

SeenCudaStatus seen_cuda_graph_instantiate(SeenCudaHandle graph,
                                           SeenCudaHandle *graph_exec) {
    Graph *graph_object = checked<Graph>(graph, kGraphMagic);
    if (!graph_object || !graph_exec) return invalid("graph-instantiate", "invalid graph or output");
    *graph_exec = 0;
    auto *object = static_cast<GraphExec *>(std::calloc(1, sizeof(GraphExec)));
    if (!object) return status(SEEN_CUDA_OUT_OF_MEMORY, 0, graph_object->device,
                               "graph-instantiate", "graph executable metadata allocation failed");
    cudaError_t error = cudaGraphInstantiate(&object->value, graph_object->value, 0);
    if (error != cudaSuccess) { std::free(object); return cuda_failure("graph-instantiate", error, graph_object->device); }
    object->magic = kGraphExecMagic; object->device = graph_object->device;
    *graph_exec = reinterpret_cast<uintptr_t>(object);
    return ok("graph-instantiate", object->device);
}

SeenCudaStatus seen_cuda_graph_launch(SeenCudaHandle graph_exec,
                                      SeenCudaHandle stream) {
    GraphExec *exec_object = checked<GraphExec>(graph_exec, kGraphExecMagic);
    Stream *stream_object = checked<Stream>(stream, kStreamMagic);
    if (!exec_object || !stream_object || exec_object->device != stream_object->device)
        return invalid("graph-launch", "invalid or cross-device graph/stream");
    cudaError_t error = cudaGraphLaunch(exec_object->value, stream_object->value);
    return error == cudaSuccess ? ok("graph-launch", exec_object->device)
                                : cuda_failure("graph-launch", error, exec_object->device);
}

SeenCudaStatus seen_cuda_graph_exec_update(SeenCudaHandle graph_exec,
    SeenCudaHandle graph, int32_t *updated) {
    GraphExec *exec_object = checked<GraphExec>(graph_exec, kGraphExecMagic);
    Graph *graph_object = checked<Graph>(graph, kGraphMagic);
    if (!exec_object || !graph_object || !updated ||
        exec_object->device != graph_object->device)
        return invalid("graph-exec-update", "invalid or cross-device graph update");
    cudaGraphExecUpdateResultInfo info{};
    cudaError_t error = cudaGraphExecUpdate(exec_object->value,
                                            graph_object->value, &info);
    *updated = error == cudaSuccess && info.result == cudaGraphExecUpdateSuccess;
    return error == cudaSuccess ? ok("graph-exec-update", exec_object->device)
                                : cuda_failure("graph-exec-update", error, exec_object->device);
}

SeenCudaStatus seen_cuda_graph_exec_destroy(SeenCudaHandle *graph_exec) {
    if (!graph_exec) return invalid("graph-exec-destroy", "missing owner handle");
    if (*graph_exec == 0) return ok("graph-exec-destroy");
    GraphExec *object = checked<GraphExec>(*graph_exec, kGraphExecMagic);
    if (!object) return invalid("graph-exec-destroy", "invalid graph executable owner");
    cudaError_t error = cudaGraphExecDestroy(object->value);
    if (error != cudaSuccess) return cuda_failure("graph-exec-destroy", error, object->device);
    object->magic = 0; std::free(object); *graph_exec = 0;
    return ok("graph-exec-destroy");
}

SeenCudaStatus seen_cuda_graph_destroy(SeenCudaHandle *graph) {
    if (!graph) return invalid("graph-destroy", "missing owner handle");
    if (*graph == 0) return ok("graph-destroy");
    Graph *object = checked<Graph>(*graph, kGraphMagic);
    if (!object) return invalid("graph-destroy", "invalid graph owner");
    cudaError_t error = cudaGraphDestroy(object->value);
    if (error != cudaSuccess) return cuda_failure("graph-destroy", error, object->device);
    object->magic = 0; std::free(object); *graph = 0;
    return ok("graph-destroy");
}

SeenCudaStatus seen_cublaslt_create(int32_t device, SeenCudaHandle *handle) {
    if (!handle) return invalid("cublaslt-create", "missing output");
    *handle = 0;
    SeenCudaStatus selected = select_device(device, "cublaslt-create");
    if (selected.code != SEEN_CUDA_OK) return selected;
    auto *object = static_cast<LtHandle *>(std::calloc(1, sizeof(LtHandle)));
    if (!object) return status(SEEN_CUDA_OUT_OF_MEMORY, 0, device,
                               "cublaslt-create", "handle metadata allocation failed");
    cublasStatus_t error = cublasLtCreate(&object->value);
    if (error != CUBLAS_STATUS_SUCCESS) { std::free(object); return cublas_failure("cublaslt-create", error, device); }
    object->magic = kLtMagic; object->device = device;
    *handle = reinterpret_cast<uintptr_t>(object);
    return ok("cublaslt-create", device);
}

SeenCudaStatus seen_cublaslt_destroy(SeenCudaHandle *handle) {
    if (!handle) return invalid("cublaslt-destroy", "missing owner handle");
    if (*handle == 0) return ok("cublaslt-destroy");
    LtHandle *object = checked<LtHandle>(*handle, kLtMagic);
    if (!object) return invalid("cublaslt-destroy", "invalid cuBLASLt owner");
    cublasStatus_t error = cublasLtDestroy(object->value);
    if (error != CUBLAS_STATUS_SUCCESS) return cublas_failure("cublaslt-destroy", error, object->device);
    object->magic = 0; std::free(object); *handle = 0;
    return ok("cublaslt-destroy");
}

SeenCudaStatus seen_cublaslt_select_algorithm(SeenCudaHandle handle,
    const SeenCudaMatmulDesc *descriptor, SeenCudaAlgorithm *algorithm) {
    LtHandle *object = checked<LtHandle>(handle, kLtMagic);
    if (!object) return invalid("cublaslt-select-algorithm", "invalid cuBLASLt handle");
    return select_algorithm_internal(object, descriptor, algorithm, nullptr);
}

SeenCudaStatus seen_cublaslt_matmul(SeenCudaHandle handle,
    const SeenCudaMatmulDesc *descriptor, const SeenCudaAlgorithm *algorithm,
    const void *a, const void *b, void *c, void *workspace,
    SeenCudaHandle stream) {
    LtHandle *handle_object = checked<LtHandle>(handle, kLtMagic);
    Stream *stream_object = checked<Stream>(stream, kStreamMagic);
    if (!handle_object || !stream_object || !algorithm || !a || !b || !c ||
        handle_object->device != stream_object->device)
        return invalid("cublaslt-matmul", "invalid or cross-device matmul resources");
    SeenCudaAlgorithm selected_algorithm{};
    cublasLtMatmulHeuristicResult_t selected{};
    SeenCudaStatus selection = select_algorithm_internal(handle_object, descriptor,
        &selected_algorithm, &selected);
    if (selection.code != SEEN_CUDA_OK) return selection;
    if (selected_algorithm.cache_identity != algorithm->cache_identity ||
        algorithm->workspace_bytes > descriptor->workspace_limit_bytes ||
        (algorithm->workspace_bytes > 0 && !workspace))
        return status(SEEN_CUDA_INCOMPATIBLE, 0, handle_object->device,
            "cublaslt-matmul", "algorithm cache identity or workspace is incompatible");
    LtDescriptors d;
    cublasStatus_t error = create_descriptors(descriptor, &d);
    if (error != CUBLAS_STATUS_SUCCESS) {
        destroy_descriptors(&d);
        return cublas_failure("cublaslt-matmul", error, handle_object->device);
    }
    const float alpha = 1.0f;
    const float beta = 0.0f;
    error = cublasLtMatmul(handle_object->value, d.operation, &alpha,
        a, d.a, b, d.b, &beta, c, d.c, c, d.c, &selected.algo,
        workspace, static_cast<size_t>(algorithm->workspace_bytes),
        stream_object->value);
    destroy_descriptors(&d);
    return error == CUBLAS_STATUS_SUCCESS
        ? ok("cublaslt-matmul", handle_object->device)
        : cublas_failure("cublaslt-matmul", error, handle_object->device);
}

}  // extern "C"
