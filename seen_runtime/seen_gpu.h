// Seen GPU Runtime - Vulkan compute dispatch
// Provides GPU buffer management, shader loading, pipeline creation, and dispatch
// Linked with -lvulkan when GPU functions are used

#ifndef SEEN_GPU_H
#define SEEN_GPU_H

#include <stdint.h>

#include "seen_runtime.h"

#ifdef __cplusplus
extern "C" {
#endif

// Initialize Vulkan GPU context (instance, device, queue, command pool)
// Returns 1 on success, 0 on failure
int64_t seen_gpu_init(void);

// Shut down GPU context and free all Vulkan resources
void seen_gpu_shutdown(void);

// Check if GPU is initialized and available
// Returns 1 if available, 0 otherwise
int64_t seen_gpu_is_available(void);

// Return selected Vulkan physical device type.
// Values mirror VkPhysicalDeviceType: 1=integrated, 2=discrete, 3=virtual, 4=cpu.
// Returns -1 when no Vulkan device is initialized.
int64_t seen_gpu_device_type(void);

// Create a GPU buffer
// usage: 0=storage, 1=uniform, 2=indirect
// Returns buffer handle (as i64), or 0 on failure
int64_t seen_gpu_buffer_create(int64_t size, int64_t usage);

// Write data from CPU memory to GPU buffer
// Returns 1 on success, 0 on failure
int64_t seen_gpu_buffer_write(int64_t handle, void* data, int64_t size);

// Read data from GPU buffer to CPU memory
// Returns 1 on success, 0 on failure
int64_t seen_gpu_buffer_read(int64_t handle, void* data, int64_t size);

// Seen Array helpers for stdlib wrappers. Array<Float> is double-backed on the
// CPU side; these helpers convert to/from f32 storage used by GLSL buffers.
int64_t seen_gpu_buffer_write_float_array(int64_t handle, void* array_ptr, int64_t count);
int64_t seen_gpu_buffer_read_float_array(int64_t handle, void* array_ptr, int64_t count);
int64_t seen_gpu_buffer_write_int_array(int64_t handle, void* array_ptr, int64_t count);
int64_t seen_gpu_buffer_read_int_array(int64_t handle, void* array_ptr, int64_t count);
int64_t seen_gpu_buffer_write_f32_scalar(int64_t handle, double value);
int64_t seen_gpu_buffer_write_i32_scalar(int64_t handle, int64_t value);

// Destroy a GPU buffer and free its memory
void seen_gpu_buffer_destroy(int64_t handle);

// Load a SPIR-V shader from file
// Returns shader module handle (as i64), or 0 on failure
int64_t seen_gpu_shader_load(SeenString spirv_path);

// Create a compute pipeline from a shader module
// Returns pipeline handle (as i64), or 0 on failure
int64_t seen_gpu_pipeline_create(int64_t shader_handle, int64_t binding_count);

// Destroy a compute pipeline and all associated resources
void seen_gpu_pipeline_destroy(int64_t handle);

// Dispatch a compute shader
// buffers: array of buffer handles, buffer_count: number of buffers
// Returns 1 on success, 0 on failure
int64_t seen_gpu_dispatch(int64_t pipeline_handle, int64_t gx, int64_t gy, int64_t gz,
                          int64_t* buffers, int64_t buffer_count);

// Fixed-arity dispatch helper for Seen code, which cannot safely pass a raw
// pointer to an Array<Int> handle buffer yet.
int64_t seen_gpu_dispatch_handles(int64_t pipeline_handle, int64_t gx, int64_t gy, int64_t gz,
                                  int64_t h0, int64_t h1, int64_t h2, int64_t h3,
                                  int64_t h4, int64_t h5, int64_t h6, int64_t h7,
                                  int64_t buffer_count);

// Dispatch a compute shader with indirect dispatch buffer
// Returns 1 on success, 0 on failure
int64_t seen_gpu_dispatch_indirect(int64_t pipeline_handle, int64_t indirect_buf_handle,
                                   int64_t* buffers, int64_t buffer_count);

// Create a fence for CPU/GPU synchronization
// Returns fence handle (as i64), or 0 on failure
int64_t seen_gpu_fence_create(void);

// Wait for a fence to be signaled
// timeout_ns: timeout in nanoseconds (0 = no wait, UINT64_MAX = infinite)
// Returns 1 if signaled, 0 if timeout
int64_t seen_gpu_fence_wait(int64_t handle, int64_t timeout_ns);

// Destroy a fence
void seen_gpu_fence_destroy(int64_t handle);

// Wait for all GPU operations to complete
// Returns 1 on success, 0 on failure
int64_t seen_gpu_device_wait_idle(void);

// CPU-side barrier (no-op for CPU execution of GPU code)
void seen_barrier(void);

#ifdef __cplusplus
}
#endif

#endif // SEEN_GPU_H
