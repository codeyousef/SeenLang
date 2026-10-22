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
int main(void) {
    SIZE(VkApplicationInfo, 48); SIZE(VkInstanceCreateInfo, 64);
    SIZE(VkDeviceQueueCreateInfo, 40); SIZE(VkDeviceCreateInfo, 72);
    SIZE(VkPhysicalDeviceProperties, 824); SIZE(VkPhysicalDeviceProperties2, 840);
    SIZE(VkPhysicalDeviceIDProperties, 64); SIZE(VkPhysicalDeviceSubgroupProperties, 32);
    SIZE(VkPhysicalDeviceMemoryProperties, 520); SIZE(VkPhysicalDeviceFeatures, 220);
    SIZE(VkQueueFamilyProperties, 24);
    SIZE(VkBufferCreateInfo, 56); SIZE(VkMemoryAllocateInfo, 32); SIZE(VkMemoryRequirements, 24);
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
