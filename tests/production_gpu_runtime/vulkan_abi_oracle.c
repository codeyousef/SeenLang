/* Test-only SDK oracle for Seen's direct Vulkan ABI declarations. */
#include <stddef.h>
#include <stdio.h>
#include <vulkan/vulkan.h>

#define SIZE(type, expected) _Static_assert(sizeof(type) == (expected), #type " size"); printf(#type "=%zu\n", sizeof(type))
#define FIELD(type, field, expected) _Static_assert(offsetof(type, field) == (expected), #type "." #field " offset"); printf(#type "." #field "=%zu\n", offsetof(type, field))
_Static_assert(VK_API_VERSION_1_2 == 4202496, "Vulkan 1.2 version encoding");
_Static_assert(sizeof(void *) == 8, "64-bit Vulkan ABI required");
_Static_assert(VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2 == 1000059001, "properties sType");
_Static_assert(VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_ID_PROPERTIES == 1000071004, "identity sType");
_Static_assert(VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SUBGROUP_PROPERTIES == 1000094000, "subgroup sType");
_Static_assert(VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2 == 1000059000, "features2 sType");
_Static_assert(VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_TIMELINE_SEMAPHORE_FEATURES == 1000207000, "timeline feature sType");
int main(void) {
    SIZE(VkApplicationInfo, 48); SIZE(VkInstanceCreateInfo, 64);
    SIZE(VkDeviceQueueCreateInfo, 40); SIZE(VkDeviceCreateInfo, 72);
    SIZE(VkPhysicalDeviceProperties, 824); SIZE(VkPhysicalDeviceProperties2, 840);
    SIZE(VkPhysicalDeviceIDProperties, 64); SIZE(VkPhysicalDeviceSubgroupProperties, 32);
    SIZE(VkPhysicalDeviceMemoryProperties, 520); SIZE(VkPhysicalDeviceFeatures, 220);
    SIZE(VkQueueFamilyProperties, 24);
    SIZE(VkPhysicalDeviceFeatures2, 240); SIZE(VkPhysicalDeviceTimelineSemaphoreFeatures, 24);
    FIELD(VkPhysicalDeviceFeatures2, pNext, 8); FIELD(VkPhysicalDeviceFeatures2, features, 16);
    FIELD(VkPhysicalDeviceTimelineSemaphoreFeatures, timelineSemaphore, 16);
    SIZE(VkBufferCreateInfo, 56); SIZE(VkMemoryAllocateInfo, 32); SIZE(VkMemoryRequirements, 24);
    SIZE(VkMappedMemoryRange, 40);
    SIZE(VkCommandPoolCreateInfo, 24); SIZE(VkCommandBufferAllocateInfo, 32);
    SIZE(VkCommandBufferBeginInfo, 32); SIZE(VkSemaphoreTypeCreateInfo, 32);
    SIZE(VkSemaphoreCreateInfo, 24); SIZE(VkBufferCopy, 24);
    SIZE(VkBufferMemoryBarrier, 56); SIZE(VkTimelineSemaphoreSubmitInfo, 48);
    SIZE(VkSubmitInfo, 72); SIZE(VkSemaphoreWaitInfo, 40);
    FIELD(VkCommandPoolCreateInfo, queueFamilyIndex, 20);
    FIELD(VkCommandBufferAllocateInfo, commandPool, 16);
    FIELD(VkCommandBufferAllocateInfo, commandBufferCount, 28);
    FIELD(VkCommandBufferBeginInfo, pInheritanceInfo, 24);
    FIELD(VkSemaphoreTypeCreateInfo, semaphoreType, 16);
    FIELD(VkSemaphoreTypeCreateInfo, initialValue, 24);
    FIELD(VkBufferMemoryBarrier, srcAccessMask, 16);
    FIELD(VkBufferMemoryBarrier, dstQueueFamilyIndex, 28);
    FIELD(VkBufferMemoryBarrier, buffer, 32); FIELD(VkBufferMemoryBarrier, size, 48);
    FIELD(VkTimelineSemaphoreSubmitInfo, waitSemaphoreValueCount, 16);
    FIELD(VkTimelineSemaphoreSubmitInfo, pWaitSemaphoreValues, 24);
    FIELD(VkTimelineSemaphoreSubmitInfo, signalSemaphoreValueCount, 32);
    FIELD(VkTimelineSemaphoreSubmitInfo, pSignalSemaphoreValues, 40);
    FIELD(VkSubmitInfo, pCommandBuffers, 48); FIELD(VkSubmitInfo, pSignalSemaphores, 64);
    FIELD(VkSemaphoreWaitInfo, pSemaphores, 24); FIELD(VkSemaphoreWaitInfo, pValues, 32);
    _Static_assert(VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO == 39, "copy pool sType");
    _Static_assert(VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO == 40, "copy allocate sType");
    _Static_assert(VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO == 42, "copy begin sType");
    _Static_assert(VK_STRUCTURE_TYPE_BUFFER_MEMORY_BARRIER == 44, "copy barrier sType");
    _Static_assert(VK_STRUCTURE_TYPE_SEMAPHORE_TYPE_CREATE_INFO == 1000207002, "copy timeline sType");
    _Static_assert(VK_STRUCTURE_TYPE_TIMELINE_SEMAPHORE_SUBMIT_INFO == 1000207003, "copy submit timeline sType");
    _Static_assert(VK_STRUCTURE_TYPE_SEMAPHORE_WAIT_INFO == 1000207004, "copy wait sType");
    FIELD(VkMappedMemoryRange, pNext, 8); FIELD(VkMappedMemoryRange, memory, 16);
    FIELD(VkMappedMemoryRange, offset, 24); FIELD(VkMappedMemoryRange, size, 32);
    _Static_assert(VK_STRUCTURE_TYPE_MAPPED_MEMORY_RANGE == 6, "mapped range sType");
    _Static_assert(VK_WHOLE_SIZE == UINT64_MAX, "whole mapped allocation encoding");
    FIELD(VkBufferCreateInfo, size, 24); FIELD(VkBufferCreateInfo, usage, 32);
    FIELD(VkBufferCreateInfo, queueFamilyIndexCount, 40); FIELD(VkBufferCreateInfo, pQueueFamilyIndices, 48);
    FIELD(VkMemoryAllocateInfo, allocationSize, 16); FIELD(VkMemoryAllocateInfo, memoryTypeIndex, 24);
    FIELD(VkPhysicalDeviceLimits, minStorageBufferOffsetAlignment, 328);
    FIELD(VkPhysicalDeviceProperties2, properties, 16);
    FIELD(VkPhysicalDeviceProperties, deviceName, 20);
    FIELD(VkPhysicalDeviceProperties, limits, 296);
    FIELD(VkPhysicalDeviceLimits, maxComputeSharedMemorySize, 216);
    FIELD(VkPhysicalDeviceLimits, maxComputeWorkGroupInvocations, 232);
    FIELD(VkPhysicalDeviceLimits, maxComputeWorkGroupSize, 236);
    FIELD(VkPhysicalDeviceFeatures, shaderFloat64, 156);
    FIELD(VkPhysicalDeviceFeatures, shaderInt64, 160);
    FIELD(VkPhysicalDeviceMemoryProperties, memoryHeapCount, 260);
    FIELD(VkPhysicalDeviceMemoryProperties, memoryHeaps, 264);
    printf("VkPhysicalDeviceProperties2.sType=%d\n", VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2);
    printf("VkPhysicalDeviceIDProperties.sType=%d\n", VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_ID_PROPERTIES);
    printf("VkPhysicalDeviceSubgroupProperties.sType=%d\n", VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SUBGROUP_PROPERTIES);
    return sizeof(void *) == 8 ? 0 : 1;
}
