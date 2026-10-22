/* Test-only vendor ABI fault interposition. Normal calls go to Mesa; selected
 * calls return bounded SDK faults. No production GPU policy lives here. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <vulkan/vulkan.h>

static int fault(const char *name) {
    const char *mode = getenv("SEEN_VULKAN_FIXTURE_FAULT");
    return mode && strcmp(mode, name) == 0;
}
#define NEXT(name) PFN_##name next = (PFN_##name)dlsym(RTLD_NEXT, #name); if (!next) abort()

static uint64_t buffer_ids[64], memory_ids[64];
static void track(uint64_t *ids, uint64_t value, int adding) {
    int available = -1;
    for (int i = 0; i < 64; ++i) {
        if (ids[i] == value && value) {
            if (adding) abort();
            ids[i] = 0; return;
        }
        if (!ids[i]) available = i;
    }
    if (!adding || !value || available < 0) abort();
    ids[available] = value;
}
__attribute__((destructor)) static void require_native_drain(void) {
    for (int i = 0; i < 64; ++i) {
        if (buffer_ids[i] || memory_ids[i]) {
            fputs("FAIL: Vulkan fixture retained native buffer or memory ownership\n", stderr);
            _Exit(97);
        }
    }
}

VKAPI_ATTR VkResult VKAPI_CALL vkEnumerateInstanceVersion(uint32_t *version) {
    if (fault("version-status")) return VK_ERROR_INCOMPATIBLE_DRIVER;
    if (fault("version-old")) { *version = VK_API_VERSION_1_0; return VK_SUCCESS; }
    NEXT(vkEnumerateInstanceVersion); return next(version);
}
VKAPI_ATTR VkResult VKAPI_CALL vkCreateInstance(const VkInstanceCreateInfo *info,
    const VkAllocationCallbacks *allocator, VkInstance *instance) {
    if (fault("instance-status")) return VK_ERROR_OUT_OF_HOST_MEMORY;
    if (fault("instance-empty")) { *instance = VK_NULL_HANDLE; return VK_SUCCESS; }
    NEXT(vkCreateInstance); return next(info, allocator, instance);
}
VKAPI_ATTR VkResult VKAPI_CALL vkEnumeratePhysicalDevices(VkInstance instance,
    uint32_t *count, VkPhysicalDevice *devices) {
    if (!devices && fault("devices-count-status")) return VK_ERROR_DEVICE_LOST;
    if (!devices && fault("devices-limit")) { *count = 129; return VK_SUCCESS; }
    if (devices && fault("devices-fill-status")) return VK_INCOMPLETE;
    if (devices && fault("devices-overflow")) { ++*count; return VK_SUCCESS; }
    NEXT(vkEnumeratePhysicalDevices); return next(instance, count, devices);
}
VKAPI_ATTR void VKAPI_CALL vkGetPhysicalDeviceQueueFamilyProperties(VkPhysicalDevice physical,
    uint32_t *count, VkQueueFamilyProperties *properties) {
    if (!properties && fault("queue-limit")) { *count = 129; return; }
    if (properties && fault("queue-overflow")) { ++*count; return; }
    NEXT(vkGetPhysicalDeviceQueueFamilyProperties); next(physical, count, properties);
    if (properties && fault("queue-none")) {
        for (uint32_t i = 0; i < *count; ++i) properties[i].queueFlags = VK_QUEUE_TRANSFER_BIT;
    }
}
VKAPI_ATTR VkResult VKAPI_CALL vkCreateDevice(VkPhysicalDevice physical,
    const VkDeviceCreateInfo *info, const VkAllocationCallbacks *allocator, VkDevice *device) {
    if (fault("context-status")) return VK_ERROR_OUT_OF_DEVICE_MEMORY;
    if (fault("context-empty")) { *device = VK_NULL_HANDLE; return VK_SUCCESS; }
    NEXT(vkCreateDevice); return next(physical, info, allocator, device);
}
VKAPI_ATTR void VKAPI_CALL vkGetPhysicalDeviceProperties2(VkPhysicalDevice physical,
    VkPhysicalDeviceProperties2 *properties) {
    NEXT(vkGetPhysicalDeviceProperties2); next(physical, properties);
    if (fault("binding-alignment-zero")) properties->properties.limits.minStorageBufferOffsetAlignment = 0;
}
VKAPI_ATTR void VKAPI_CALL vkGetPhysicalDeviceMemoryProperties(VkPhysicalDevice physical,
    VkPhysicalDeviceMemoryProperties *properties) {
    NEXT(vkGetPhysicalDeviceMemoryProperties); next(physical, properties);
    if (fault("memory-kind")) {
        for (uint32_t i = 0; i < properties->memoryTypeCount; ++i)
            properties->memoryTypes[i].propertyFlags =
                properties->memoryHeaps[properties->memoryTypes[i].heapIndex].flags & VK_MEMORY_HEAP_DEVICE_LOCAL_BIT;
    }
    if (fault("memory-heap")) {
        properties->memoryHeapCount = 2;
        properties->memoryHeaps[0] = (VkMemoryHeap){32768, VK_MEMORY_HEAP_DEVICE_LOCAL_BIT};
        properties->memoryHeaps[1] = properties->memoryHeaps[0];
        properties->memoryTypeCount = 2;
        properties->memoryTypes[0] = (VkMemoryType){VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT, 0};
        properties->memoryTypes[1] = (VkMemoryType){VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT, 1};
    }
}
VKAPI_ATTR VkResult VKAPI_CALL vkCreateBuffer(VkDevice device, const VkBufferCreateInfo *info,
    const VkAllocationCallbacks *allocator, VkBuffer *buffer) {
    if (fault("buffer-status")) return VK_ERROR_OUT_OF_HOST_MEMORY;
    if (fault("buffer-empty")) { *buffer = VK_NULL_HANDLE; return VK_SUCCESS; }
    NEXT(vkCreateBuffer); VkResult status = next(device, info, allocator, buffer);
    if (status == VK_SUCCESS && *buffer) track(buffer_ids, (uint64_t)(uintptr_t)*buffer, 1);
    return status;
}
VKAPI_ATTR void VKAPI_CALL vkGetBufferMemoryRequirements(VkDevice device, VkBuffer buffer,
    VkMemoryRequirements *requirements) {
    NEXT(vkGetBufferMemoryRequirements); next(device, buffer, requirements);
    if (fault("requirements-size")) requirements->size = (VkDeviceSize)1 << 63;
    if (fault("requirements-align-zero")) requirements->alignment = 0;
    if (fault("requirements-align-three")) requirements->alignment = 3;
    if (fault("requirements-types")) requirements->memoryTypeBits = 0;
}
VKAPI_ATTR VkResult VKAPI_CALL vkAllocateMemory(VkDevice device, const VkMemoryAllocateInfo *info,
    const VkAllocationCallbacks *allocator, VkDeviceMemory *memory) {
    if (fault("memory-status")) return VK_ERROR_OUT_OF_DEVICE_MEMORY;
    if (fault("memory-empty")) { *memory = VK_NULL_HANDLE; return VK_SUCCESS; }
    NEXT(vkAllocateMemory); VkResult status = next(device, info, allocator, memory);
    if (status == VK_SUCCESS && *memory) track(memory_ids, (uint64_t)(uintptr_t)*memory, 1);
    return status;
}
VKAPI_ATTR VkResult VKAPI_CALL vkBindBufferMemory(VkDevice device, VkBuffer buffer,
    VkDeviceMemory memory, VkDeviceSize offset) {
    if (fault("bind-status")) return VK_ERROR_DEVICE_LOST;
    NEXT(vkBindBufferMemory); return next(device, buffer, memory, offset);
}
VKAPI_ATTR void VKAPI_CALL vkDestroyBuffer(VkDevice device, VkBuffer buffer,
    const VkAllocationCallbacks *allocator) {
    if (buffer) track(buffer_ids, (uint64_t)(uintptr_t)buffer, 0);
    NEXT(vkDestroyBuffer); next(device, buffer, allocator);
}
VKAPI_ATTR void VKAPI_CALL vkFreeMemory(VkDevice device, VkDeviceMemory memory,
    const VkAllocationCallbacks *allocator) {
    if (memory) track(memory_ids, (uint64_t)(uintptr_t)memory, 0);
    NEXT(vkFreeMemory); next(device, memory, allocator);
}
