/* SDK-only compile/run oracle; NEVER linked into the Seen canary.
 * Internal declarations expose the deliberately selected legacy version symbols,
 * independently of CUDA 13's cuCtxCreate_v4/cuEventElapsedTime_v2 macros. */
#define __CUDA_API_VERSION_INTERNAL
#include <cuda.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <sys/resource.h>

_Static_assert(sizeof(void *) == 8 && sizeof(size_t) == 8, "Linux LP64 required");
_Static_assert(_Generic(&fread, size_t (*)(void *, size_t, size_t, FILE *): 1, default: 0),
               "bounded reflection byte-read ABI");
_Static_assert(sizeof(struct rusage) == 144 && offsetof(struct rusage, ru_maxrss) == 32,
               "Linux x86-64 resource observation layout");
_Static_assert(sizeof(((struct rusage *)0)->ru_maxrss) == 8, "RSS signed long width");
_Static_assert(sizeof(int) == 4 && sizeof(unsigned int) == 4, "Driver word ABI");
_Static_assert(sizeof(CUresult) == 4 && sizeof(CUdevice) == 4, "Driver enum/device ABI");
_Static_assert(sizeof(float) == 4 && sizeof(CUdeviceptr) == 8, "f32/deviceptr ABI");
_Static_assert(sizeof(CUcontext) == 8 && sizeof(CUmodule) == 8 && sizeof(CUfunction) == 8,
               "opaque pointer handle ABI");
_Static_assert(sizeof(CUstream) == 8 && sizeof(CUevent) == 8, "stream/event pointer ABI");
_Static_assert(sizeof(CUuuid) == 16 && offsetof(CUuuid, bytes) == 0, "UUID byte layout");
_Static_assert(CUDA_SUCCESS == 0 && CUDA_ERROR_INVALID_VALUE == 1 && CUDA_ERROR_NOT_READY == 600,
               "native status identities");
_Static_assert(CU_STREAM_NON_BLOCKING == 1 && CU_EVENT_DEFAULT == 0, "creation flags");
_Static_assert(CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MAJOR == 75 &&
               CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MINOR == 76, "SM attribute identities");
#define SIGNATURE(name, ...) \
    _Static_assert(_Generic(&(name), CUresult (*)(__VA_ARGS__): 1, default: 0), #name " signature")
SIGNATURE(cuInit, unsigned int);
SIGNATURE(cuDriverGetVersion, int *);
SIGNATURE(cuDeviceGetCount, int *);
SIGNATURE(cuDeviceGet, CUdevice *, int);
SIGNATURE(cuDeviceGetName, char *, int, CUdevice);
SIGNATURE(cuDeviceGetUuid_v2, CUuuid *, CUdevice);
SIGNATURE(cuDeviceGetAttribute, int *, CUdevice_attribute, CUdevice);
SIGNATURE(cuCtxCreate_v2, CUcontext *, unsigned int, CUdevice);
SIGNATURE(cuCtxDestroy_v2, CUcontext);
SIGNATURE(cuModuleLoad, CUmodule *, const char *);
SIGNATURE(cuModuleUnload, CUmodule);
SIGNATURE(cuModuleGetFunction, CUfunction *, CUmodule, const char *);
SIGNATURE(cuFuncGetParamInfo, CUfunction, size_t, size_t *, size_t *);
SIGNATURE(cuMemAlloc_v2, CUdeviceptr *, size_t);
SIGNATURE(cuMemFree_v2, CUdeviceptr);
SIGNATURE(cuMemcpyHtoD_v2, CUdeviceptr, const void *, size_t);
SIGNATURE(cuMemcpyDtoH_v2, void *, CUdeviceptr, size_t);
SIGNATURE(cuStreamCreate, CUstream *, unsigned int);
SIGNATURE(cuStreamQuery, CUstream);
SIGNATURE(cuStreamDestroy_v2, CUstream);
SIGNATURE(cuEventCreate, CUevent *, unsigned int);
SIGNATURE(cuEventRecord, CUevent, CUstream);
SIGNATURE(cuEventQuery, CUevent);
SIGNATURE(cuEventElapsedTime, float *, CUevent, CUevent);
SIGNATURE(cuEventDestroy_v2, CUevent);
SIGNATURE(cuLaunchKernel, CUfunction, unsigned int, unsigned int, unsigned int,
          unsigned int, unsigned int, unsigned int, unsigned int, CUstream, void **, void **);
int main(void) {
    printf("{\"schema\":\"seen-cuda-foundation-sdk-abi-v1\",\"cuda_header_version\":%d,"
           "\"pointer_bytes\":%zu,\"size_t_bytes\":%zu,\"deviceptr_bytes\":%zu,"
           "\"native_status_bytes\":%zu,\"uuid_bytes\":%zu,\"kernel_parameter_count\":6,"
           "\"kernel_packed_bytes\":56,\"native_gpu_execution\":false}\n",
           CUDA_VERSION, sizeof(void *), sizeof(size_t), sizeof(CUdeviceptr),
           sizeof(CUresult), sizeof(CUuuid));
    return 0;
}
