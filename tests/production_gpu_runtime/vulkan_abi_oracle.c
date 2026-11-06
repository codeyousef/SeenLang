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
    FIELD(VkQueueFamilyProperties, timestampValidBits, 8);
    FIELD(VkPhysicalDeviceLimits, timestampComputeAndGraphics, 420);
    FIELD(VkPhysicalDeviceLimits, timestampPeriod, 424);
    _Static_assert(offsetof(VkPhysicalDeviceProperties2, properties) +
        offsetof(VkPhysicalDeviceProperties, limits) +
        offsetof(VkPhysicalDeviceLimits, timestampPeriod) == 736, "timestamp period output offset");
    SIZE(VkQueryPoolCreateInfo, 32);
    FIELD(VkQueryPoolCreateInfo, pNext, 8);
    FIELD(VkQueryPoolCreateInfo, flags, 16);
    FIELD(VkQueryPoolCreateInfo, queryType, 20);
    FIELD(VkQueryPoolCreateInfo, queryCount, 24);
    FIELD(VkQueryPoolCreateInfo, pipelineStatistics, 28);
    _Static_assert(VK_STRUCTURE_TYPE_QUERY_POOL_CREATE_INFO == 11, "query pool sType");
    _Static_assert(VK_QUERY_TYPE_TIMESTAMP == 2, "timestamp query type");
    _Static_assert(VK_QUERY_RESULT_64_BIT == 1 &&
        VK_QUERY_RESULT_WITH_AVAILABILITY_BIT == 4 && VK_QUERY_RESULT_WAIT_BIT == 2, "timestamp result flags");
    _Static_assert(VK_PIPELINE_STAGE_TOP_OF_PIPE_BIT == 1 &&
        VK_PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT == 8192, "timestamp stages");
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
    /* Runtime H's negative probes call these real SDK layouts directly. */
    SIZE(VkFenceCreateInfo, 24); SIZE(VkFence, 8);
    FIELD(VkFenceCreateInfo, sType, 0);
    FIELD(VkFenceCreateInfo, pNext, 8); FIELD(VkFenceCreateInfo, flags, 16);
    _Static_assert(VK_STRUCTURE_TYPE_FENCE_CREATE_INFO == 8, "H fence probe sType");
    _Static_assert(VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO == 12, "H buffer probe sType");
    FIELD(VkBufferCreateInfo, sType, 0); FIELD(VkBufferCreateInfo, pNext, 8);
    FIELD(VkBufferCreateInfo, flags, 16); FIELD(VkBufferCreateInfo, sharingMode, 36);
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
    _Static_assert((VK_BUFFER_USAGE_TRANSFER_SRC_BIT | VK_BUFFER_USAGE_TRANSFER_DST_BIT |
        VK_BUFFER_USAGE_UNIFORM_BUFFER_BIT | VK_BUFFER_USAGE_STORAGE_BUFFER_BIT |
        VK_BUFFER_USAGE_INDIRECT_BUFFER_BIT) == 307, "owned typed buffer usage encoding");
    SIZE(VkShaderModuleCreateInfo, 40); FIELD(VkShaderModuleCreateInfo, codeSize, 24);
    FIELD(VkShaderModuleCreateInfo, pCode, 32);
    SIZE(VkDescriptorSetLayoutBinding, 24); FIELD(VkDescriptorSetLayoutBinding, pImmutableSamplers, 16);
    SIZE(VkDescriptorSetLayoutCreateInfo, 32); FIELD(VkDescriptorSetLayoutCreateInfo, pBindings, 24);
    SIZE(VkPipelineLayoutCreateInfo, 48); FIELD(VkPipelineLayoutCreateInfo, pSetLayouts, 24);
    FIELD(VkPipelineLayoutCreateInfo, pPushConstantRanges, 40);
    SIZE(VkPipelineShaderStageCreateInfo, 48); FIELD(VkPipelineShaderStageCreateInfo, module, 24);
    FIELD(VkPipelineShaderStageCreateInfo, pName, 32); FIELD(VkPipelineShaderStageCreateInfo, pSpecializationInfo, 40);
    SIZE(VkComputePipelineCreateInfo, 96); FIELD(VkComputePipelineCreateInfo, stage, 24);
    FIELD(VkComputePipelineCreateInfo, layout, 72); FIELD(VkComputePipelineCreateInfo, basePipelineIndex, 88);
    SIZE(VkPipelineCacheCreateInfo, 40); FIELD(VkPipelineCacheCreateInfo, initialDataSize, 24);
    FIELD(VkPipelineCacheCreateInfo, pInitialData, 32);
    FIELD(VkPipelineCacheCreateInfo, sType, 0);
    FIELD(VkPipelineCacheCreateInfo, pNext, 8); FIELD(VkPipelineCacheCreateInfo, flags, 16);
    SIZE(VkDescriptorPoolSize, 8); SIZE(VkDescriptorPoolCreateInfo, 40);
    FIELD(VkDescriptorPoolCreateInfo, pPoolSizes, 32);
    SIZE(VkDescriptorSetAllocateInfo, 40); FIELD(VkDescriptorSetAllocateInfo, pSetLayouts, 32);
    SIZE(VkDescriptorBufferInfo, 24); FIELD(VkDescriptorBufferInfo, range, 16);
    SIZE(VkWriteDescriptorSet, 64); FIELD(VkWriteDescriptorSet, dstSet, 16);
    FIELD(VkWriteDescriptorSet, pBufferInfo, 48);
    _Static_assert(VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO == 16, "shader module sType");
    _Static_assert(VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO == 18, "shader stage sType");
    _Static_assert(VK_STRUCTURE_TYPE_COMPUTE_PIPELINE_CREATE_INFO == 29, "compute pipeline sType");
    _Static_assert(VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO == 30, "pipeline layout sType");
    _Static_assert(VK_STRUCTURE_TYPE_PIPELINE_CACHE_CREATE_INFO == 17, "pipeline cache sType");
    _Static_assert(VK_STRUCTURE_TYPE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO == 32, "descriptor layout sType");
    _Static_assert(VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO == 33, "descriptor pool sType");
    _Static_assert(VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO == 34, "descriptor allocation sType");
    _Static_assert(VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET == 35, "descriptor write sType");
    FIELD(VkPhysicalDeviceProperties, pipelineCacheUUID, 276);
    FIELD(VkPhysicalDeviceLimits, maxUniformBufferRange, 24);
    FIELD(VkPhysicalDeviceLimits, maxStorageBufferRange, 28);
    FIELD(VkPhysicalDeviceLimits, maxBoundDescriptorSets, 64);
    FIELD(VkPhysicalDeviceLimits, maxPerStageDescriptorUniformBuffers, 72);
    FIELD(VkPhysicalDeviceLimits, maxPerStageDescriptorStorageBuffers, 76);
    FIELD(VkPhysicalDeviceLimits, maxDescriptorSetUniformBuffers, 100);
    FIELD(VkPhysicalDeviceLimits, maxDescriptorSetStorageBuffers, 108);
    FIELD(VkPhysicalDeviceLimits, maxComputeWorkGroupCount, 220);
    FIELD(VkPhysicalDeviceLimits, minUniformBufferOffsetAlignment, 320);
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
