// Seen GPU Runtime - Vulkan compute dispatch implementation
// Provides complete GPU buffer management, shader loading, pipeline creation, and dispatch
// Requires Vulkan SDK (libvulkan)

#include "seen_gpu.h"
#include "seen_runtime.h"
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// Conditionally include Vulkan headers
#ifdef __has_include
#if __has_include(<vulkan/vulkan.h>)
#define SEEN_HAS_VULKAN 1
#include <vulkan/vulkan.h>
#else
#define SEEN_HAS_VULKAN 0
#endif
#else
// Try to include anyway, will fail at compile time if not available
#define SEEN_HAS_VULKAN 1
#include <vulkan/vulkan.h>
#endif

#if SEEN_HAS_VULKAN

// --- Internal Structures ---

typedef struct {
    VkBuffer buffer;
    VkDeviceMemory memory;
    VkDeviceSize size;
    void* mapped;
    int host_visible;
} SeenGpuBuffer;

typedef struct {
    VkPipeline pipeline;
    VkPipelineLayout layout;
    VkDescriptorSetLayout desc_layout;
    VkDescriptorPool desc_pool;
    VkShaderModule shader_module;
    int binding_count;
} SeenGpuPipeline;

typedef struct {
    VkInstance instance;
    VkPhysicalDevice physical_device;
    VkDevice device;
    VkQueue compute_queue;
    uint32_t compute_queue_family;
    VkCommandPool command_pool;
    VkPhysicalDeviceMemoryProperties mem_props;
    int device_type;
    int initialized;
} SeenGpuContext;

// Global singleton context
static SeenGpuContext g_gpu = {0};

// --- Helper Functions ---

static int seen_gpu_mul_bytes(size_t count, size_t item_size, size_t* out_bytes) {
    if (item_size != 0 && count > SIZE_MAX / item_size) {
        return 0;
    }
    size_t bytes = count * item_size;
    if (bytes > (size_t)INT64_MAX) {
        return 0;
    }
    *out_bytes = bytes;
    return 1;
}

static void* seen_gpu_alloc_bytes(size_t bytes, const char* label) {
    if (bytes == 0 || bytes > (size_t)INT64_MAX) {
        fprintf(stderr, "[seen_gpu] Invalid allocation size for %s: %zu\n", label, bytes);
        return NULL;
    }
    void* ptr = seen_try_malloc((int64_t)bytes);
    if (!ptr) {
        fprintf(stderr, "[seen_gpu] Allocation budget exhausted for %s (%zu bytes)\n", label, bytes);
    }
    return ptr;
}

static void* seen_gpu_calloc_bytes(size_t count, size_t item_size, const char* label, size_t* out_bytes) {
    size_t bytes = 0;
    if (!seen_gpu_mul_bytes(count, item_size, &bytes) || bytes == 0) {
        fprintf(stderr, "[seen_gpu] Invalid allocation size for %s\n", label);
        return NULL;
    }
    void* ptr = seen_try_calloc((int64_t)count, (int64_t)item_size);
    if (!ptr) {
        fprintf(stderr, "[seen_gpu] Allocation budget exhausted for %s (%zu bytes)\n", label, bytes);
        return NULL;
    }
    if (out_bytes) {
        *out_bytes = bytes;
    }
    return ptr;
}

static void seen_gpu_free_bytes(void* ptr, size_t bytes) {
    if (!ptr) return;
    if (bytes > 0 && bytes <= (size_t)INT64_MAX) {
        seen_memory_release_bytes((int64_t)bytes);
    }
    free(ptr);
}

static uint32_t find_memory_type(uint32_t type_filter, VkMemoryPropertyFlags properties) {
    for (uint32_t i = 0; i < g_gpu.mem_props.memoryTypeCount; i++) {
        if ((type_filter & (1 << i)) &&
            (g_gpu.mem_props.memoryTypes[i].propertyFlags & properties) == properties) {
            return i;
        }
    }
    return UINT32_MAX;
}

static int create_vk_buffer(VkDeviceSize size, VkBufferUsageFlags usage,
                            VkMemoryPropertyFlags mem_props,
                            VkBuffer* out_buffer, VkDeviceMemory* out_memory) {
    VkBufferCreateInfo buf_info = {0};
    buf_info.sType = VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO;
    buf_info.size = size;
    buf_info.usage = usage;
    buf_info.sharingMode = VK_SHARING_MODE_EXCLUSIVE;

    VkResult res = vkCreateBuffer(g_gpu.device, &buf_info, NULL, out_buffer);
    if (res != VK_SUCCESS) {
        fprintf(stderr, "[seen_gpu] vkCreateBuffer failed: %d\n", res);
        return 0;
    }

    VkMemoryRequirements mem_reqs;
    vkGetBufferMemoryRequirements(g_gpu.device, *out_buffer, &mem_reqs);

    uint32_t mem_type = find_memory_type(mem_reqs.memoryTypeBits, mem_props);
    if (mem_type == UINT32_MAX) {
        fprintf(stderr, "[seen_gpu] No suitable memory type found\n");
        vkDestroyBuffer(g_gpu.device, *out_buffer, NULL);
        return 0;
    }

    VkMemoryAllocateInfo alloc_info = {0};
    alloc_info.sType = VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO;
    alloc_info.allocationSize = mem_reqs.size;
    alloc_info.memoryTypeIndex = mem_type;

    res = vkAllocateMemory(g_gpu.device, &alloc_info, NULL, out_memory);
    if (res != VK_SUCCESS) {
        fprintf(stderr, "[seen_gpu] vkAllocateMemory failed: %d\n", res);
        vkDestroyBuffer(g_gpu.device, *out_buffer, NULL);
        return 0;
    }

    res = vkBindBufferMemory(g_gpu.device, *out_buffer, *out_memory, 0);
    if (res != VK_SUCCESS) {
        fprintf(stderr, "[seen_gpu] vkBindBufferMemory failed: %d\n", res);
        vkFreeMemory(g_gpu.device, *out_memory, NULL);
        vkDestroyBuffer(g_gpu.device, *out_buffer, NULL);
        return 0;
    }

    return 1;
}

static VkCommandBuffer begin_one_shot_cmd(void) {
    VkCommandBufferAllocateInfo alloc_info = {0};
    alloc_info.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO;
    alloc_info.commandPool = g_gpu.command_pool;
    alloc_info.level = VK_COMMAND_BUFFER_LEVEL_PRIMARY;
    alloc_info.commandBufferCount = 1;

    VkCommandBuffer cmd;
    VkResult res = vkAllocateCommandBuffers(g_gpu.device, &alloc_info, &cmd);
    if (res != VK_SUCCESS) {
        fprintf(stderr, "[seen_gpu] vkAllocateCommandBuffers failed: %d\n", res);
        return VK_NULL_HANDLE;
    }

    VkCommandBufferBeginInfo begin_info = {0};
    begin_info.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO;
    begin_info.flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT;

    res = vkBeginCommandBuffer(cmd, &begin_info);
    if (res != VK_SUCCESS) {
        fprintf(stderr, "[seen_gpu] vkBeginCommandBuffer failed: %d\n", res);
        vkFreeCommandBuffers(g_gpu.device, g_gpu.command_pool, 1, &cmd);
        return VK_NULL_HANDLE;
    }

    return cmd;
}

static int end_and_submit_cmd(VkCommandBuffer cmd) {
    VkResult res = vkEndCommandBuffer(cmd);
    if (res != VK_SUCCESS) {
        fprintf(stderr, "[seen_gpu] vkEndCommandBuffer failed: %d\n", res);
        vkFreeCommandBuffers(g_gpu.device, g_gpu.command_pool, 1, &cmd);
        return 0;
    }

    VkFenceCreateInfo fence_info = {0};
    fence_info.sType = VK_STRUCTURE_TYPE_FENCE_CREATE_INFO;
    VkFence fence;
    res = vkCreateFence(g_gpu.device, &fence_info, NULL, &fence);
    if (res != VK_SUCCESS) {
        fprintf(stderr, "[seen_gpu] vkCreateFence failed: %d\n", res);
        vkFreeCommandBuffers(g_gpu.device, g_gpu.command_pool, 1, &cmd);
        return 0;
    }

    VkSubmitInfo submit_info = {0};
    submit_info.sType = VK_STRUCTURE_TYPE_SUBMIT_INFO;
    submit_info.commandBufferCount = 1;
    submit_info.pCommandBuffers = &cmd;

    res = vkQueueSubmit(g_gpu.compute_queue, 1, &submit_info, fence);
    if (res != VK_SUCCESS) {
        fprintf(stderr, "[seen_gpu] vkQueueSubmit failed: %d\n", res);
        vkDestroyFence(g_gpu.device, fence, NULL);
        vkFreeCommandBuffers(g_gpu.device, g_gpu.command_pool, 1, &cmd);
        return 0;
    }

    res = vkWaitForFences(g_gpu.device, 1, &fence, VK_TRUE, UINT64_MAX);
    vkDestroyFence(g_gpu.device, fence, NULL);
    vkFreeCommandBuffers(g_gpu.device, g_gpu.command_pool, 1, &cmd);

    if (res != VK_SUCCESS) {
        fprintf(stderr, "[seen_gpu] vkWaitForFences failed: %d\n", res);
        return 0;
    }

    return 1;
}

// --- Public API ---

int64_t seen_gpu_init(void) {
    if (g_gpu.initialized) {
        return 1;
    }

    // Create Vulkan instance
    VkApplicationInfo app_info = {0};
    app_info.sType = VK_STRUCTURE_TYPE_APPLICATION_INFO;
    app_info.pApplicationName = "Seen GPU Runtime";
    app_info.applicationVersion = VK_MAKE_VERSION(1, 0, 0);
    app_info.pEngineName = "Seen";
    app_info.engineVersion = VK_MAKE_VERSION(1, 0, 0);
    app_info.apiVersion = VK_API_VERSION_1_0;

    VkInstanceCreateInfo inst_info = {0};
    inst_info.sType = VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO;
    inst_info.pApplicationInfo = &app_info;

    VkResult res = vkCreateInstance(&inst_info, NULL, &g_gpu.instance);
    if (res != VK_SUCCESS) {
        fprintf(stderr, "[seen_gpu] vkCreateInstance failed: %d\n", res);
        return 0;
    }

    // Enumerate physical devices
    uint32_t device_count = 0;
    vkEnumeratePhysicalDevices(g_gpu.instance, &device_count, NULL);
    if (device_count == 0) {
        fprintf(stderr, "[seen_gpu] No Vulkan physical devices found\n");
        vkDestroyInstance(g_gpu.instance, NULL);
        memset(&g_gpu, 0, sizeof(g_gpu));
        return 0;
    }

    size_t devices_bytes = 0;
    if (!seen_gpu_mul_bytes((size_t)device_count, sizeof(VkPhysicalDevice), &devices_bytes)) {
        fprintf(stderr, "[seen_gpu] Physical device list is too large\n");
        vkDestroyInstance(g_gpu.instance, NULL);
        memset(&g_gpu, 0, sizeof(g_gpu));
        return 0;
    }
    VkPhysicalDevice* devices = (VkPhysicalDevice*)seen_gpu_alloc_bytes(devices_bytes, "physical device list");
    if (!devices) {
        vkDestroyInstance(g_gpu.instance, NULL);
        memset(&g_gpu, 0, sizeof(g_gpu));
        return 0;
    }
    res = vkEnumeratePhysicalDevices(g_gpu.instance, &device_count, devices);
    if (res != VK_SUCCESS) {
        fprintf(stderr, "[seen_gpu] vkEnumeratePhysicalDevices failed: %d\n", res);
        seen_gpu_free_bytes(devices, devices_bytes);
        vkDestroyInstance(g_gpu.instance, NULL);
        memset(&g_gpu, 0, sizeof(g_gpu));
        return 0;
    }

    // Prefer discrete GPU
    g_gpu.physical_device = devices[0];
    for (uint32_t i = 0; i < device_count; i++) {
        VkPhysicalDeviceProperties props;
        vkGetPhysicalDeviceProperties(devices[i], &props);
        if (props.deviceType == VK_PHYSICAL_DEVICE_TYPE_DISCRETE_GPU) {
            g_gpu.physical_device = devices[i];
            break;
        }
    }
    seen_gpu_free_bytes(devices, devices_bytes);

    VkPhysicalDeviceProperties selected_props;
    vkGetPhysicalDeviceProperties(g_gpu.physical_device, &selected_props);
    g_gpu.device_type = (int)selected_props.deviceType;

    // Find compute queue family
    uint32_t queue_family_count = 0;
    vkGetPhysicalDeviceQueueFamilyProperties(g_gpu.physical_device, &queue_family_count, NULL);
    size_t queue_families_bytes = 0;
    if (!seen_gpu_mul_bytes((size_t)queue_family_count, sizeof(VkQueueFamilyProperties), &queue_families_bytes)) {
        fprintf(stderr, "[seen_gpu] Queue family list is too large\n");
        vkDestroyInstance(g_gpu.instance, NULL);
        memset(&g_gpu, 0, sizeof(g_gpu));
        return 0;
    }
    VkQueueFamilyProperties* queue_families =
        (VkQueueFamilyProperties*)seen_gpu_alloc_bytes(queue_families_bytes, "queue family list");
    if (!queue_families) {
        vkDestroyInstance(g_gpu.instance, NULL);
        memset(&g_gpu, 0, sizeof(g_gpu));
        return 0;
    }
    vkGetPhysicalDeviceQueueFamilyProperties(g_gpu.physical_device, &queue_family_count, queue_families);

    g_gpu.compute_queue_family = UINT32_MAX;
    for (uint32_t i = 0; i < queue_family_count; i++) {
        if (queue_families[i].queueFlags & VK_QUEUE_COMPUTE_BIT) {
            g_gpu.compute_queue_family = i;
            // Prefer a dedicated compute queue (not graphics)
            if (!(queue_families[i].queueFlags & VK_QUEUE_GRAPHICS_BIT)) {
                break;
            }
        }
    }
    seen_gpu_free_bytes(queue_families, queue_families_bytes);

    if (g_gpu.compute_queue_family == UINT32_MAX) {
        fprintf(stderr, "[seen_gpu] No compute queue family found\n");
        vkDestroyInstance(g_gpu.instance, NULL);
        memset(&g_gpu, 0, sizeof(g_gpu));
        return 0;
    }

    // Create logical device with compute queue
    float queue_priority = 1.0f;
    VkDeviceQueueCreateInfo queue_info = {0};
    queue_info.sType = VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO;
    queue_info.queueFamilyIndex = g_gpu.compute_queue_family;
    queue_info.queueCount = 1;
    queue_info.pQueuePriorities = &queue_priority;

    VkDeviceCreateInfo dev_info = {0};
    dev_info.sType = VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO;
    dev_info.queueCreateInfoCount = 1;
    dev_info.pQueueCreateInfos = &queue_info;

    res = vkCreateDevice(g_gpu.physical_device, &dev_info, NULL, &g_gpu.device);
    if (res != VK_SUCCESS) {
        fprintf(stderr, "[seen_gpu] vkCreateDevice failed: %d\n", res);
        vkDestroyInstance(g_gpu.instance, NULL);
        memset(&g_gpu, 0, sizeof(g_gpu));
        return 0;
    }

    // Get compute queue
    vkGetDeviceQueue(g_gpu.device, g_gpu.compute_queue_family, 0, &g_gpu.compute_queue);

    // Create transient command pool
    VkCommandPoolCreateInfo pool_info = {0};
    pool_info.sType = VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO;
    pool_info.queueFamilyIndex = g_gpu.compute_queue_family;
    pool_info.flags = VK_COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BIT;

    res = vkCreateCommandPool(g_gpu.device, &pool_info, NULL, &g_gpu.command_pool);
    if (res != VK_SUCCESS) {
        fprintf(stderr, "[seen_gpu] vkCreateCommandPool failed: %d\n", res);
        vkDestroyDevice(g_gpu.device, NULL);
        vkDestroyInstance(g_gpu.instance, NULL);
        memset(&g_gpu, 0, sizeof(g_gpu));
        return 0;
    }

    // Cache memory properties
    vkGetPhysicalDeviceMemoryProperties(g_gpu.physical_device, &g_gpu.mem_props);

    g_gpu.initialized = 1;
    return 1;
}

void seen_gpu_shutdown(void) {
    if (!g_gpu.initialized) return;

    vkDeviceWaitIdle(g_gpu.device);
    vkDestroyCommandPool(g_gpu.device, g_gpu.command_pool, NULL);
    vkDestroyDevice(g_gpu.device, NULL);
    vkDestroyInstance(g_gpu.instance, NULL);
    memset(&g_gpu, 0, sizeof(g_gpu));
}

int64_t seen_gpu_is_available(void) {
    return g_gpu.initialized ? 1 : 0;
}

int64_t seen_gpu_device_type(void) {
    if (!g_gpu.initialized) return -1;
    return (int64_t)g_gpu.device_type;
}

int64_t seen_gpu_buffer_create(int64_t size, int64_t usage) {
    if (!g_gpu.initialized) return 0;

    size_t buffer_handle_bytes = 0;
    SeenGpuBuffer* buf = (SeenGpuBuffer*)seen_gpu_calloc_bytes(
        1, sizeof(SeenGpuBuffer), "GPU buffer handle", &buffer_handle_bytes);
    if (!buf) return 0;
    buf->size = (VkDeviceSize)size;

    VkBufferUsageFlags vk_usage = VK_BUFFER_USAGE_TRANSFER_SRC_BIT | VK_BUFFER_USAGE_TRANSFER_DST_BIT;
    if (usage == 0) {
        vk_usage |= VK_BUFFER_USAGE_STORAGE_BUFFER_BIT;
    } else if (usage == 1) {
        vk_usage |= VK_BUFFER_USAGE_UNIFORM_BUFFER_BIT;
    } else if (usage == 2) {
        vk_usage |= VK_BUFFER_USAGE_INDIRECT_BUFFER_BIT | VK_BUFFER_USAGE_STORAGE_BUFFER_BIT;
    }

    // Try host-visible + host-coherent memory first (most convenient for CPU access)
    VkMemoryPropertyFlags mem_flags = VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT;
    int ok = create_vk_buffer(size, vk_usage, mem_flags, &buf->buffer, &buf->memory);

    if (ok) {
        buf->host_visible = 1;
        // Map the memory
        VkResult res = vkMapMemory(g_gpu.device, buf->memory, 0, size, 0, &buf->mapped);
        if (res != VK_SUCCESS) {
            buf->mapped = NULL;
        }
    } else {
        // Fall back to device-local memory
        mem_flags = VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT;
        ok = create_vk_buffer(size, vk_usage, mem_flags, &buf->buffer, &buf->memory);
        if (!ok) {
            seen_gpu_free_bytes(buf, buffer_handle_bytes);
            return 0;
        }
        buf->host_visible = 0;
        buf->mapped = NULL;
    }

    return (int64_t)(uintptr_t)buf;
}

int64_t seen_gpu_buffer_write(int64_t handle, void* data, int64_t size) {
    if (!g_gpu.initialized || !handle || !data) return 0;
    SeenGpuBuffer* buf = (SeenGpuBuffer*)(uintptr_t)handle;

    if (buf->host_visible && buf->mapped) {
        memcpy(buf->mapped, data, (size_t)size);
        return 1;
    }

    // Device-local: use staging buffer
    VkBuffer staging_buf;
    VkDeviceMemory staging_mem;
    VkMemoryPropertyFlags staging_flags = VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT;
    if (!create_vk_buffer(size, VK_BUFFER_USAGE_TRANSFER_SRC_BIT, staging_flags, &staging_buf, &staging_mem)) {
        return 0;
    }

    void* staging_mapped;
    VkResult res = vkMapMemory(g_gpu.device, staging_mem, 0, size, 0, &staging_mapped);
    if (res != VK_SUCCESS) {
        vkDestroyBuffer(g_gpu.device, staging_buf, NULL);
        vkFreeMemory(g_gpu.device, staging_mem, NULL);
        return 0;
    }
    memcpy(staging_mapped, data, (size_t)size);
    vkUnmapMemory(g_gpu.device, staging_mem);

    VkCommandBuffer cmd = begin_one_shot_cmd();
    if (!cmd) {
        vkDestroyBuffer(g_gpu.device, staging_buf, NULL);
        vkFreeMemory(g_gpu.device, staging_mem, NULL);
        return 0;
    }

    VkBufferCopy copy_region = {0};
    copy_region.size = size;
    vkCmdCopyBuffer(cmd, staging_buf, buf->buffer, 1, &copy_region);

    int submit_ok = end_and_submit_cmd(cmd);
    vkDestroyBuffer(g_gpu.device, staging_buf, NULL);
    vkFreeMemory(g_gpu.device, staging_mem, NULL);

    return submit_ok ? 1 : 0;
}

int64_t seen_gpu_buffer_read(int64_t handle, void* data, int64_t size) {
    if (!g_gpu.initialized || !handle || !data) return 0;
    SeenGpuBuffer* buf = (SeenGpuBuffer*)(uintptr_t)handle;

    if (buf->host_visible && buf->mapped) {
        memcpy(data, buf->mapped, (size_t)size);
        return 1;
    }

    // Device-local: use staging buffer
    VkBuffer staging_buf;
    VkDeviceMemory staging_mem;
    VkMemoryPropertyFlags staging_flags = VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT;
    if (!create_vk_buffer(size, VK_BUFFER_USAGE_TRANSFER_DST_BIT, staging_flags, &staging_buf, &staging_mem)) {
        return 0;
    }

    VkCommandBuffer cmd = begin_one_shot_cmd();
    if (!cmd) {
        vkDestroyBuffer(g_gpu.device, staging_buf, NULL);
        vkFreeMemory(g_gpu.device, staging_mem, NULL);
        return 0;
    }

    VkBufferCopy copy_region = {0};
    copy_region.size = size;
    vkCmdCopyBuffer(cmd, buf->buffer, staging_buf, 1, &copy_region);

    int submit_ok = end_and_submit_cmd(cmd);
    if (!submit_ok) {
        vkDestroyBuffer(g_gpu.device, staging_buf, NULL);
        vkFreeMemory(g_gpu.device, staging_mem, NULL);
        return 0;
    }

    void* staging_mapped;
    VkResult res = vkMapMemory(g_gpu.device, staging_mem, 0, size, 0, &staging_mapped);
    if (res != VK_SUCCESS) {
        vkDestroyBuffer(g_gpu.device, staging_buf, NULL);
        vkFreeMemory(g_gpu.device, staging_mem, NULL);
        return 0;
    }
    memcpy(data, staging_mapped, (size_t)size);
    vkUnmapMemory(g_gpu.device, staging_mem);

    vkDestroyBuffer(g_gpu.device, staging_buf, NULL);
    vkFreeMemory(g_gpu.device, staging_mem, NULL);

    return 1;
}

static int64_t seen_gpu_clamped_array_count(SeenArray* arr, int64_t requested) {
    if (!arr || requested < 0 || arr->len < 0) {
        return -1;
    }
    if (requested == 0 || requested > arr->len) {
        return arr->len;
    }
    return requested;
}

int64_t seen_gpu_buffer_write_float_array(int64_t handle, void* array_ptr, int64_t count) {
    SeenArray* arr = (SeenArray*)array_ptr;
    int64_t n = seen_gpu_clamped_array_count(arr, count);
    if (n < 0 || !arr->data) return 0;
    size_t byte_count = 0;
    if (!seen_gpu_mul_bytes((size_t)n, sizeof(float), &byte_count)) return 0;
    if (byte_count == 0) return 1;

    float* tmp = (float*)seen_gpu_alloc_bytes(byte_count, "GPU f32 upload staging");
    if (!tmp) return 0;
    double* src = (double*)arr->data;
    for (int64_t i = 0; i < n; i++) {
        tmp[i] = (float)src[i];
    }
    int64_t ok = seen_gpu_buffer_write(handle, tmp, (int64_t)byte_count);
    seen_gpu_free_bytes(tmp, byte_count);
    return ok;
}

int64_t seen_gpu_buffer_read_float_array(int64_t handle, void* array_ptr, int64_t count) {
    SeenArray* arr = (SeenArray*)array_ptr;
    int64_t n = seen_gpu_clamped_array_count(arr, count);
    if (n < 0 || !arr->data) return 0;
    size_t byte_count = 0;
    if (!seen_gpu_mul_bytes((size_t)n, sizeof(float), &byte_count)) return 0;
    if (byte_count == 0) return 1;

    float* tmp = (float*)seen_gpu_alloc_bytes(byte_count, "GPU f32 readback staging");
    if (!tmp) return 0;
    int64_t ok = seen_gpu_buffer_read(handle, tmp, (int64_t)byte_count);
    if (ok == 1) {
        double* dst = (double*)arr->data;
        for (int64_t i = 0; i < n; i++) {
            dst[i] = (double)tmp[i];
        }
    }
    seen_gpu_free_bytes(tmp, byte_count);
    return ok;
}

int64_t seen_gpu_buffer_write_int_array(int64_t handle, void* array_ptr, int64_t count) {
    SeenArray* arr = (SeenArray*)array_ptr;
    int64_t n = seen_gpu_clamped_array_count(arr, count);
    if (n < 0 || !arr->data) return 0;
    size_t byte_count = 0;
    if (!seen_gpu_mul_bytes((size_t)n, sizeof(int32_t), &byte_count)) return 0;
    if (byte_count == 0) return 1;

    int32_t* tmp = (int32_t*)seen_gpu_alloc_bytes(byte_count, "GPU i32 upload staging");
    if (!tmp) return 0;
    int64_t* src = (int64_t*)arr->data;
    for (int64_t i = 0; i < n; i++) {
        tmp[i] = (int32_t)src[i];
    }
    int64_t ok = seen_gpu_buffer_write(handle, tmp, (int64_t)byte_count);
    seen_gpu_free_bytes(tmp, byte_count);
    return ok;
}

int64_t seen_gpu_buffer_read_int_array(int64_t handle, void* array_ptr, int64_t count) {
    SeenArray* arr = (SeenArray*)array_ptr;
    int64_t n = seen_gpu_clamped_array_count(arr, count);
    if (n < 0 || !arr->data) return 0;
    size_t byte_count = 0;
    if (!seen_gpu_mul_bytes((size_t)n, sizeof(int32_t), &byte_count)) return 0;
    if (byte_count == 0) return 1;

    int32_t* tmp = (int32_t*)seen_gpu_alloc_bytes(byte_count, "GPU i32 readback staging");
    if (!tmp) return 0;
    int64_t ok = seen_gpu_buffer_read(handle, tmp, (int64_t)byte_count);
    if (ok == 1) {
        int64_t* dst = (int64_t*)arr->data;
        for (int64_t i = 0; i < n; i++) {
            dst[i] = (int64_t)tmp[i];
        }
    }
    seen_gpu_free_bytes(tmp, byte_count);
    return ok;
}

int64_t seen_gpu_buffer_write_f32_scalar(int64_t handle, double value) {
    float v = (float)value;
    return seen_gpu_buffer_write(handle, &v, (int64_t)sizeof(float));
}

int64_t seen_gpu_buffer_write_i32_scalar(int64_t handle, int64_t value) {
    int32_t v = (int32_t)value;
    return seen_gpu_buffer_write(handle, &v, (int64_t)sizeof(int32_t));
}

void seen_gpu_buffer_destroy(int64_t handle) {
    if (!g_gpu.initialized || !handle) return;
    SeenGpuBuffer* buf = (SeenGpuBuffer*)(uintptr_t)handle;

    if (buf->mapped) {
        vkUnmapMemory(g_gpu.device, buf->memory);
    }
    vkDestroyBuffer(g_gpu.device, buf->buffer, NULL);
    vkFreeMemory(g_gpu.device, buf->memory, NULL);
    seen_gpu_free_bytes(buf, sizeof(SeenGpuBuffer));
}

int64_t seen_gpu_shader_load(SeenString spirv_path_seen) {
    if (!g_gpu.initialized || spirv_path_seen.len <= 0 || !spirv_path_seen.data) return 0;

    char* spirv_path = seen_str_to_cstr(spirv_path_seen);
    if (!spirv_path) return 0;

    FILE* f = fopen(spirv_path, "rb");
    if (!f) {
        fprintf(stderr, "[seen_gpu] Cannot open shader: %s\n", spirv_path);
        free(spirv_path);
        return 0;
    }

    fseek(f, 0, SEEK_END);
    long file_size = ftell(f);
    fseek(f, 0, SEEK_SET);

    if (file_size <= 0 || file_size % 4 != 0) {
        fprintf(stderr, "[seen_gpu] Invalid SPIR-V file: %s (size=%ld)\n", spirv_path, file_size);
        fclose(f);
        free(spirv_path);
        return 0;
    }

    if ((uint64_t)file_size > (uint64_t)INT64_MAX) {
        fprintf(stderr, "[seen_gpu] SPIR-V file too large: %s (size=%ld)\n", spirv_path, file_size);
        fclose(f);
        free(spirv_path);
        return 0;
    }

    size_t code_bytes = (size_t)file_size;
    uint32_t* code = (uint32_t*)seen_gpu_alloc_bytes(code_bytes, "SPIR-V shader bytes");
    if (!code) {
        fclose(f);
        free(spirv_path);
        return 0;
    }
    size_t read_size = fread(code, 1, file_size, f);
    fclose(f);

    if ((long)read_size != file_size) {
        fprintf(stderr, "[seen_gpu] Failed to read shader: %s\n", spirv_path);
        seen_gpu_free_bytes(code, code_bytes);
        free(spirv_path);
        return 0;
    }

    VkShaderModuleCreateInfo module_info = {0};
    module_info.sType = VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO;
    module_info.codeSize = file_size;
    module_info.pCode = code;

    VkShaderModule shader_module;
    VkResult res = vkCreateShaderModule(g_gpu.device, &module_info, NULL, &shader_module);
    seen_gpu_free_bytes(code, code_bytes);

    if (res != VK_SUCCESS) {
        fprintf(stderr, "[seen_gpu] vkCreateShaderModule failed: %d\n", res);
        free(spirv_path);
        return 0;
    }

    free(spirv_path);
    return (int64_t)(uintptr_t)shader_module;
}

int64_t seen_gpu_pipeline_create(int64_t shader_handle, int64_t binding_count) {
    if (!g_gpu.initialized || !shader_handle) return 0;
    VkShaderModule shader_module = (VkShaderModule)(uintptr_t)shader_handle;

    if (binding_count < 0 || binding_count > INT_MAX || binding_count > UINT32_MAX) {
        fprintf(stderr, "[seen_gpu] Invalid descriptor binding count: %" PRId64 "\n", binding_count);
        return 0;
    }

    size_t pipeline_bytes = 0;
    SeenGpuPipeline* pipe = (SeenGpuPipeline*)seen_gpu_calloc_bytes(
        1, sizeof(SeenGpuPipeline), "GPU pipeline handle", &pipeline_bytes);
    if (!pipe) return 0;
    pipe->shader_module = shader_module;
    pipe->binding_count = (int)binding_count;

    // Create descriptor set layout with N storage buffer bindings
    VkDescriptorSetLayoutBinding* bindings = NULL;
    size_t bindings_bytes = 0;
    if (binding_count > 0) {
        bindings = (VkDescriptorSetLayoutBinding*)seen_gpu_calloc_bytes(
            (size_t)binding_count, sizeof(VkDescriptorSetLayoutBinding),
            "GPU descriptor bindings", &bindings_bytes);
        if (!bindings) {
            seen_gpu_free_bytes(pipe, pipeline_bytes);
            return 0;
        }
        for (int i = 0; i < binding_count; i++) {
            bindings[i].binding = i;
            bindings[i].descriptorType = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER;
            bindings[i].descriptorCount = 1;
            bindings[i].stageFlags = VK_SHADER_STAGE_COMPUTE_BIT;
        }
    }

    VkDescriptorSetLayoutCreateInfo layout_info = {0};
    layout_info.sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO;
    layout_info.bindingCount = (uint32_t)binding_count;
    layout_info.pBindings = bindings;

    VkResult res = vkCreateDescriptorSetLayout(g_gpu.device, &layout_info, NULL, &pipe->desc_layout);
    seen_gpu_free_bytes(bindings, bindings_bytes);
    if (res != VK_SUCCESS) {
        fprintf(stderr, "[seen_gpu] vkCreateDescriptorSetLayout failed: %d\n", res);
        seen_gpu_free_bytes(pipe, pipeline_bytes);
        return 0;
    }

    // Create pipeline layout
    VkPipelineLayoutCreateInfo pl_info = {0};
    pl_info.sType = VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO;
    pl_info.setLayoutCount = 1;
    pl_info.pSetLayouts = &pipe->desc_layout;

    res = vkCreatePipelineLayout(g_gpu.device, &pl_info, NULL, &pipe->layout);
    if (res != VK_SUCCESS) {
        fprintf(stderr, "[seen_gpu] vkCreatePipelineLayout failed: %d\n", res);
        vkDestroyDescriptorSetLayout(g_gpu.device, pipe->desc_layout, NULL);
        seen_gpu_free_bytes(pipe, pipeline_bytes);
        return 0;
    }

    // Create compute pipeline
    VkPipelineShaderStageCreateInfo stage_info = {0};
    stage_info.sType = VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO;
    stage_info.stage = VK_SHADER_STAGE_COMPUTE_BIT;
    stage_info.module = shader_module;
    stage_info.pName = "main";

    VkComputePipelineCreateInfo comp_info = {0};
    comp_info.sType = VK_STRUCTURE_TYPE_COMPUTE_PIPELINE_CREATE_INFO;
    comp_info.stage = stage_info;
    comp_info.layout = pipe->layout;

    res = vkCreateComputePipelines(g_gpu.device, VK_NULL_HANDLE, 1, &comp_info, NULL, &pipe->pipeline);
    if (res != VK_SUCCESS) {
        fprintf(stderr, "[seen_gpu] vkCreateComputePipelines failed: %d\n", res);
        vkDestroyPipelineLayout(g_gpu.device, pipe->layout, NULL);
        vkDestroyDescriptorSetLayout(g_gpu.device, pipe->desc_layout, NULL);
        seen_gpu_free_bytes(pipe, pipeline_bytes);
        return 0;
    }

    // Create descriptor pool (max 1 set, N storage buffers)
    if (binding_count > 0) {
        VkDescriptorPoolSize pool_size = {0};
        pool_size.type = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER;
        pool_size.descriptorCount = (uint32_t)binding_count;

        VkDescriptorPoolCreateInfo pool_info = {0};
        pool_info.sType = VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO;
        pool_info.maxSets = 1;
        pool_info.poolSizeCount = 1;
        pool_info.pPoolSizes = &pool_size;
        pool_info.flags = VK_DESCRIPTOR_POOL_CREATE_FREE_DESCRIPTOR_SET_BIT;

        res = vkCreateDescriptorPool(g_gpu.device, &pool_info, NULL, &pipe->desc_pool);
        if (res != VK_SUCCESS) {
            fprintf(stderr, "[seen_gpu] vkCreateDescriptorPool failed: %d\n", res);
            vkDestroyPipeline(g_gpu.device, pipe->pipeline, NULL);
            vkDestroyPipelineLayout(g_gpu.device, pipe->layout, NULL);
            vkDestroyDescriptorSetLayout(g_gpu.device, pipe->desc_layout, NULL);
            seen_gpu_free_bytes(pipe, pipeline_bytes);
            return 0;
        }
    }

    return (int64_t)(uintptr_t)pipe;
}

void seen_gpu_pipeline_destroy(int64_t handle) {
    if (!g_gpu.initialized || !handle) return;
    SeenGpuPipeline* pipe = (SeenGpuPipeline*)(uintptr_t)handle;

    vkDestroyPipeline(g_gpu.device, pipe->pipeline, NULL);
    vkDestroyPipelineLayout(g_gpu.device, pipe->layout, NULL);
    vkDestroyDescriptorSetLayout(g_gpu.device, pipe->desc_layout, NULL);
    if (pipe->desc_pool) {
        vkDestroyDescriptorPool(g_gpu.device, pipe->desc_pool, NULL);
    }
    vkDestroyShaderModule(g_gpu.device, pipe->shader_module, NULL);
    seen_gpu_free_bytes(pipe, sizeof(SeenGpuPipeline));
}

int64_t seen_gpu_dispatch(int64_t pipeline_handle, int64_t gx, int64_t gy, int64_t gz,
                          int64_t* buffers, int64_t buffer_count) {
    if (!g_gpu.initialized || !pipeline_handle) return 0;
    SeenGpuPipeline* pipe = (SeenGpuPipeline*)(uintptr_t)pipeline_handle;

    // Allocate descriptor set
    VkDescriptorSet desc_set = VK_NULL_HANDLE;
    int64_t usable_count64 = buffer_count < pipe->binding_count ? buffer_count : pipe->binding_count;
    if (pipe->binding_count > 0 && usable_count64 > 0) {
        if (!buffers) {
            fprintf(stderr, "[seen_gpu] Missing buffer handles for dispatch\n");
            return 0;
        }

        VkDescriptorSetAllocateInfo alloc_info = {0};
        alloc_info.sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO;
        alloc_info.descriptorPool = pipe->desc_pool;
        alloc_info.descriptorSetCount = 1;
        alloc_info.pSetLayouts = &pipe->desc_layout;

        VkResult res = vkAllocateDescriptorSets(g_gpu.device, &alloc_info, &desc_set);
        if (res != VK_SUCCESS) {
            fprintf(stderr, "[seen_gpu] vkAllocateDescriptorSets failed: %d\n", res);
            return 0;
        }

        // Write buffer descriptors
        size_t descriptor_count = (size_t)usable_count64;
        size_t writes_bytes = 0;
        size_t buf_infos_bytes = 0;
        VkWriteDescriptorSet* writes = (VkWriteDescriptorSet*)seen_gpu_calloc_bytes(
            descriptor_count, sizeof(VkWriteDescriptorSet), "GPU descriptor writes", &writes_bytes);
        VkDescriptorBufferInfo* buf_infos = (VkDescriptorBufferInfo*)seen_gpu_calloc_bytes(
            descriptor_count, sizeof(VkDescriptorBufferInfo), "GPU descriptor buffer infos", &buf_infos_bytes);
        if (!writes || !buf_infos) {
            seen_gpu_free_bytes(writes, writes_bytes);
            seen_gpu_free_bytes(buf_infos, buf_infos_bytes);
            vkFreeDescriptorSets(g_gpu.device, pipe->desc_pool, 1, &desc_set);
            return 0;
        }

        for (int i = 0; i < usable_count64; i++) {
            SeenGpuBuffer* buf = (SeenGpuBuffer*)(uintptr_t)buffers[i];
            if (!buf) {
                fprintf(stderr, "[seen_gpu] Null GPU buffer handle at descriptor %d\n", i);
                seen_gpu_free_bytes(writes, writes_bytes);
                seen_gpu_free_bytes(buf_infos, buf_infos_bytes);
                vkFreeDescriptorSets(g_gpu.device, pipe->desc_pool, 1, &desc_set);
                return 0;
            }
            buf_infos[i].buffer = buf->buffer;
            buf_infos[i].offset = 0;
            buf_infos[i].range = VK_WHOLE_SIZE;

            writes[i].sType = VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET;
            writes[i].dstSet = desc_set;
            writes[i].dstBinding = i;
            writes[i].descriptorCount = 1;
            writes[i].descriptorType = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER;
            writes[i].pBufferInfo = &buf_infos[i];
        }

        vkUpdateDescriptorSets(g_gpu.device, (uint32_t)usable_count64, writes, 0, NULL);
        seen_gpu_free_bytes(writes, writes_bytes);
        seen_gpu_free_bytes(buf_infos, buf_infos_bytes);
    }

    // Record and submit command buffer
    VkCommandBuffer cmd = begin_one_shot_cmd();
    if (!cmd) {
        if (desc_set) {
            vkFreeDescriptorSets(g_gpu.device, pipe->desc_pool, 1, &desc_set);
        }
        return 0;
    }

    vkCmdBindPipeline(cmd, VK_PIPELINE_BIND_POINT_COMPUTE, pipe->pipeline);
    if (desc_set) {
        vkCmdBindDescriptorSets(cmd, VK_PIPELINE_BIND_POINT_COMPUTE, pipe->layout,
                                0, 1, &desc_set, 0, NULL);
    }
    vkCmdDispatch(cmd, (uint32_t)gx, (uint32_t)gy, (uint32_t)gz);

    int submit_ok = end_and_submit_cmd(cmd);

    // Reset descriptor pool for reuse
    if (pipe->desc_pool) {
        vkResetDescriptorPool(g_gpu.device, pipe->desc_pool, 0);
    }

    return submit_ok ? 1 : 0;
}

int64_t seen_gpu_dispatch_handles(int64_t pipeline_handle, int64_t gx, int64_t gy, int64_t gz,
                                  int64_t h0, int64_t h1, int64_t h2, int64_t h3,
                                  int64_t h4, int64_t h5, int64_t h6, int64_t h7,
                                  int64_t buffer_count) {
    int64_t buffers[8] = { h0, h1, h2, h3, h4, h5, h6, h7 };
    if (buffer_count < 0 || buffer_count > 8) {
        fprintf(stderr, "[seen_gpu] Invalid fixed dispatch buffer count: %" PRId64 "\n", buffer_count);
        return 0;
    }
    return seen_gpu_dispatch(pipeline_handle, gx, gy, gz, buffers, buffer_count);
}

int64_t seen_gpu_dispatch_indirect(int64_t pipeline_handle, int64_t indirect_buf_handle,
                                   int64_t* buffers, int64_t buffer_count) {
    if (!g_gpu.initialized || !pipeline_handle || !indirect_buf_handle) return 0;
    SeenGpuPipeline* pipe = (SeenGpuPipeline*)(uintptr_t)pipeline_handle;
    SeenGpuBuffer* indirect_buf = (SeenGpuBuffer*)(uintptr_t)indirect_buf_handle;

    // Allocate descriptor set
    VkDescriptorSet desc_set = VK_NULL_HANDLE;
    int64_t usable_count64 = buffer_count < pipe->binding_count ? buffer_count : pipe->binding_count;
    if (pipe->binding_count > 0 && usable_count64 > 0) {
        if (!buffers) {
            fprintf(stderr, "[seen_gpu] Missing buffer handles for indirect dispatch\n");
            return 0;
        }

        VkDescriptorSetAllocateInfo alloc_info = {0};
        alloc_info.sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO;
        alloc_info.descriptorPool = pipe->desc_pool;
        alloc_info.descriptorSetCount = 1;
        alloc_info.pSetLayouts = &pipe->desc_layout;

        VkResult res = vkAllocateDescriptorSets(g_gpu.device, &alloc_info, &desc_set);
        if (res != VK_SUCCESS) {
            fprintf(stderr, "[seen_gpu] vkAllocateDescriptorSets failed: %d\n", res);
            return 0;
        }

        size_t descriptor_count = (size_t)usable_count64;
        size_t writes_bytes = 0;
        size_t buf_infos_bytes = 0;
        VkWriteDescriptorSet* writes = (VkWriteDescriptorSet*)seen_gpu_calloc_bytes(
            descriptor_count, sizeof(VkWriteDescriptorSet), "GPU indirect descriptor writes", &writes_bytes);
        VkDescriptorBufferInfo* buf_infos = (VkDescriptorBufferInfo*)seen_gpu_calloc_bytes(
            descriptor_count, sizeof(VkDescriptorBufferInfo), "GPU indirect descriptor buffer infos", &buf_infos_bytes);
        if (!writes || !buf_infos) {
            seen_gpu_free_bytes(writes, writes_bytes);
            seen_gpu_free_bytes(buf_infos, buf_infos_bytes);
            vkFreeDescriptorSets(g_gpu.device, pipe->desc_pool, 1, &desc_set);
            return 0;
        }

        for (int i = 0; i < usable_count64; i++) {
            SeenGpuBuffer* buf = (SeenGpuBuffer*)(uintptr_t)buffers[i];
            if (!buf) {
                fprintf(stderr, "[seen_gpu] Null GPU buffer handle at indirect descriptor %d\n", i);
                seen_gpu_free_bytes(writes, writes_bytes);
                seen_gpu_free_bytes(buf_infos, buf_infos_bytes);
                vkFreeDescriptorSets(g_gpu.device, pipe->desc_pool, 1, &desc_set);
                return 0;
            }
            buf_infos[i].buffer = buf->buffer;
            buf_infos[i].offset = 0;
            buf_infos[i].range = VK_WHOLE_SIZE;

            writes[i].sType = VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET;
            writes[i].dstSet = desc_set;
            writes[i].dstBinding = i;
            writes[i].descriptorCount = 1;
            writes[i].descriptorType = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER;
            writes[i].pBufferInfo = &buf_infos[i];
        }

        vkUpdateDescriptorSets(g_gpu.device, (uint32_t)usable_count64, writes, 0, NULL);
        seen_gpu_free_bytes(writes, writes_bytes);
        seen_gpu_free_bytes(buf_infos, buf_infos_bytes);
    }

    VkCommandBuffer cmd = begin_one_shot_cmd();
    if (!cmd) {
        if (desc_set) {
            vkFreeDescriptorSets(g_gpu.device, pipe->desc_pool, 1, &desc_set);
        }
        return 0;
    }

    vkCmdBindPipeline(cmd, VK_PIPELINE_BIND_POINT_COMPUTE, pipe->pipeline);
    if (desc_set) {
        vkCmdBindDescriptorSets(cmd, VK_PIPELINE_BIND_POINT_COMPUTE, pipe->layout,
                                0, 1, &desc_set, 0, NULL);
    }
    vkCmdDispatchIndirect(cmd, indirect_buf->buffer, 0);

    int submit_ok = end_and_submit_cmd(cmd);

    if (pipe->desc_pool) {
        vkResetDescriptorPool(g_gpu.device, pipe->desc_pool, 0);
    }

    return submit_ok ? 1 : 0;
}

int64_t seen_gpu_fence_create(void) {
    if (!g_gpu.initialized) return 0;

    VkFenceCreateInfo fence_info = {0};
    fence_info.sType = VK_STRUCTURE_TYPE_FENCE_CREATE_INFO;

    VkFence fence;
    VkResult res = vkCreateFence(g_gpu.device, &fence_info, NULL, &fence);
    if (res != VK_SUCCESS) {
        fprintf(stderr, "[seen_gpu] vkCreateFence failed: %d\n", res);
        return 0;
    }

    return (int64_t)(uintptr_t)fence;
}

int64_t seen_gpu_fence_wait(int64_t handle, int64_t timeout_ns) {
    if (!g_gpu.initialized || !handle) return 0;
    VkFence fence = (VkFence)(uintptr_t)handle;

    VkResult res = vkWaitForFences(g_gpu.device, 1, &fence, VK_TRUE, (uint64_t)timeout_ns);
    if (res == VK_SUCCESS) return 1;
    if (res == VK_TIMEOUT) return 0;

    fprintf(stderr, "[seen_gpu] vkWaitForFences failed: %d\n", res);
    return 0;
}

void seen_gpu_fence_destroy(int64_t handle) {
    if (!g_gpu.initialized || !handle) return;
    VkFence fence = (VkFence)(uintptr_t)handle;
    vkDestroyFence(g_gpu.device, fence, NULL);
}

int64_t seen_gpu_device_wait_idle(void) {
    if (!g_gpu.initialized) return 0;
    VkResult res = vkDeviceWaitIdle(g_gpu.device);
    return (res == VK_SUCCESS) ? 1 : 0;
}

void seen_barrier(void) {
    // CPU-side no-op barrier for GPU code running on CPU
}

#else // !SEEN_HAS_VULKAN

// Stub implementations when Vulkan is not available
int64_t seen_gpu_init(void) {
    fprintf(stderr, "[seen_gpu] Vulkan not available at compile time\n");
    return 0;
}

void seen_gpu_shutdown(void) {}

int64_t seen_gpu_is_available(void) { return 0; }

int64_t seen_gpu_device_type(void) { return -1; }

int64_t seen_gpu_buffer_create(int64_t size, int64_t usage) {
    (void)size; (void)usage;
    return 0;
}

int64_t seen_gpu_buffer_write(int64_t handle, void* data, int64_t size) {
    (void)handle; (void)data; (void)size;
    return 0;
}

int64_t seen_gpu_buffer_read(int64_t handle, void* data, int64_t size) {
    (void)handle; (void)data; (void)size;
    return 0;
}

int64_t seen_gpu_buffer_write_float_array(int64_t handle, void* array_ptr, int64_t count) {
    (void)handle; (void)array_ptr; (void)count;
    return 0;
}

int64_t seen_gpu_buffer_read_float_array(int64_t handle, void* array_ptr, int64_t count) {
    (void)handle; (void)array_ptr; (void)count;
    return 0;
}

int64_t seen_gpu_buffer_write_int_array(int64_t handle, void* array_ptr, int64_t count) {
    (void)handle; (void)array_ptr; (void)count;
    return 0;
}

int64_t seen_gpu_buffer_read_int_array(int64_t handle, void* array_ptr, int64_t count) {
    (void)handle; (void)array_ptr; (void)count;
    return 0;
}

int64_t seen_gpu_buffer_write_f32_scalar(int64_t handle, double value) {
    (void)handle; (void)value;
    return 0;
}

int64_t seen_gpu_buffer_write_i32_scalar(int64_t handle, int64_t value) {
    (void)handle; (void)value;
    return 0;
}

void seen_gpu_buffer_destroy(int64_t handle) { (void)handle; }

int64_t seen_gpu_shader_load(SeenString spirv_path) {
    (void)spirv_path;
    return 0;
}

int64_t seen_gpu_pipeline_create(int64_t shader_handle, int64_t binding_count) {
    (void)shader_handle; (void)binding_count;
    return 0;
}

void seen_gpu_pipeline_destroy(int64_t handle) { (void)handle; }

int64_t seen_gpu_dispatch(int64_t pipeline_handle, int64_t gx, int64_t gy, int64_t gz,
                          int64_t* buffers, int64_t buffer_count) {
    (void)pipeline_handle; (void)gx; (void)gy; (void)gz;
    (void)buffers; (void)buffer_count;
    return 0;
}

int64_t seen_gpu_dispatch_handles(int64_t pipeline_handle, int64_t gx, int64_t gy, int64_t gz,
                                  int64_t h0, int64_t h1, int64_t h2, int64_t h3,
                                  int64_t h4, int64_t h5, int64_t h6, int64_t h7,
                                  int64_t buffer_count) {
    (void)pipeline_handle; (void)gx; (void)gy; (void)gz;
    (void)h0; (void)h1; (void)h2; (void)h3;
    (void)h4; (void)h5; (void)h6; (void)h7;
    (void)buffer_count;
    return 0;
}

int64_t seen_gpu_dispatch_indirect(int64_t pipeline_handle, int64_t indirect_buf_handle,
                                   int64_t* buffers, int64_t buffer_count) {
    (void)pipeline_handle; (void)indirect_buf_handle;
    (void)buffers; (void)buffer_count;
    return 0;
}

int64_t seen_gpu_fence_create(void) { return 0; }

int64_t seen_gpu_fence_wait(int64_t handle, int64_t timeout_ns) {
    (void)handle; (void)timeout_ns;
    return 0;
}

void seen_gpu_fence_destroy(int64_t handle) { (void)handle; }

int64_t seen_gpu_device_wait_idle(void) { return 0; }

void seen_barrier(void) {}

#endif // SEEN_HAS_VULKAN
