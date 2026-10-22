/* Test-only vendor ABI fault interposition. Normal calls go to Mesa; selected
 * calls return bounded SDK faults. No production GPU policy lives here. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdlib.h>
#include <string.h>
#include <vulkan/vulkan.h>

static int fault(const char *name) {
    const char *mode = getenv("SEEN_VULKAN_FIXTURE_FAULT");
    return mode && strcmp(mode, name) == 0;
}
#define NEXT(name) PFN_##name next = (PFN_##name)dlsym(RTLD_NEXT, #name); if (!next) abort()

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
