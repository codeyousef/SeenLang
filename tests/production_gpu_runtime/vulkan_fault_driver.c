/* Test-only vendor ABI fault interposition. Normal calls go to Mesa; selected
 * calls return bounded SDK faults. No production GPU policy lives here. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <time.h>
#include <sys/syscall.h>
#include <unistd.h>
#include <vulkan/vulkan.h>

static int fault(const char *name) {
    const char *mode = getenv("SEEN_VULKAN_FIXTURE_FAULT");
    return mode && strcmp(mode, name) == 0;
}
#define NEXT(name) PFN_##name next = (PFN_##name)dlsym(RTLD_NEXT, #name); if (!next) abort()

static uint64_t buffer_ids[64], memory_ids[64], mapped_ids[64];
static unsigned map_calls, unmap_calls, flush_calls, invalidate_calls, buffer_calls, memory_calls;
static uint64_t copy_pools[64], copy_semaphores[64];
static uint64_t command_ids[256], command_pools[256], command_semaphores[256], command_signals[256];
static unsigned pool_creates, pool_destroys, semaphore_creates, semaphore_destroys;
static unsigned command_allocations, command_count, copy_calls, submit_calls, poll_calls, injected_copy_fault;
static int copy_clock_late;
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
        if (buffer_ids[i] || memory_ids[i] || mapped_ids[i] || copy_pools[i] || copy_semaphores[i]) {
            fputs("FAIL: Vulkan fixture retained native buffer or memory ownership\n", stderr);
            _Exit(97);
        }
    }
    for (int i = 0; i < 256; ++i) if (command_ids[i]) { fputs("FAIL: Vulkan fixture retained native commands\n", stderr); _Exit(97); }
    if (getenv("SEEN_VULKAN_FIXTURE_MAP_TRACE") && (buffer_calls || map_calls))
        fprintf(stderr, "VULKAN_MAP_TRACE map=%u unmap=%u flush=%u invalidate=%u\n",
                map_calls, unmap_calls, flush_calls, invalidate_calls);
    if (getenv("SEEN_VULKAN_FIXTURE_COPY_TRACE") && (pool_creates || command_allocations))
        fprintf(stderr, "VULKAN_COPY_TRACE pool=%u/%u semaphore=%u/%u alloc=%u commands=%u copy=%u submit=%u buffers=%u memory=%u poll=%u drained=1\n",
                pool_creates, pool_destroys, semaphore_creates, semaphore_destroys,
                command_allocations, command_count, copy_calls, submit_calls, buffer_calls, memory_calls, poll_calls);
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
VKAPI_ATTR void VKAPI_CALL vkGetPhysicalDeviceFeatures2(VkPhysicalDevice physical,
    VkPhysicalDeviceFeatures2 *features) {
    NEXT(vkGetPhysicalDeviceFeatures2); next(physical, features);
    VkPhysicalDeviceTimelineSemaphoreFeatures *timeline = features->pNext;
    if (fault("timeline-root-tag")) features->sType = 0;
    if (fault("timeline-root-chain")) features->pNext = NULL;
    if (timeline) {
        if (fault("timeline-tag")) timeline->sType = 0;
        if (fault("timeline-chain")) timeline->pNext = (void *)(uintptr_t)1;
        if (fault("timeline-bool")) timeline->timelineSemaphore = 2;
        if (fault("timeline-none")) timeline->timelineSemaphore = VK_FALSE;
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
    if (fault("map-not-visible")) {
        for (uint32_t i = 0; i < properties->memoryTypeCount; ++i)
            properties->memoryTypes[i].propertyFlags &= ~(VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT);
    }
    if (fault("map-noncoherent") || fault("map-flush-once") || fault("map-invalidate")) {
        for (uint32_t i = 0; i < properties->memoryTypeCount; ++i)
            properties->memoryTypes[i].propertyFlags &= ~VK_MEMORY_PROPERTY_HOST_COHERENT_BIT;
    }
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
    ++buffer_calls;
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
    ++memory_calls;
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

VKAPI_ATTR VkResult VKAPI_CALL vkMapMemory(VkDevice device, VkDeviceMemory memory,
    VkDeviceSize offset, VkDeviceSize size, VkMemoryMapFlags flags, void **data) {
    ++map_calls;
    if (fault("map-status")) return VK_ERROR_MEMORY_MAP_FAILED;
    NEXT(vkMapMemory);
    VkResult result = next(device, memory, offset, size, flags, data);
    if (result == VK_SUCCESS) {
        track(mapped_ids, (uint64_t)(uintptr_t)memory, 1);
        if (fault("map-empty")) *data = NULL;
        if (fault("map-address-overflow") || fault("map-span-overflow")) *data = (void *)(UINTPTR_MAX - 4);
    }
    return result;
}
VKAPI_ATTR void VKAPI_CALL vkUnmapMemory(VkDevice device, VkDeviceMemory memory) {
    ++unmap_calls;
    track(mapped_ids, (uint64_t)(uintptr_t)memory, 0);
    NEXT(vkUnmapMemory); next(device, memory);
}
VKAPI_ATTR VkResult VKAPI_CALL vkFlushMappedMemoryRanges(VkDevice device,
    uint32_t count, const VkMappedMemoryRange *ranges) {
    ++flush_calls;
    if (count != 1 || ranges[0].sType != VK_STRUCTURE_TYPE_MAPPED_MEMORY_RANGE ||
        ranges[0].pNext || ranges[0].offset != 0 || ranges[0].size != VK_WHOLE_SIZE) abort();
    if (fault("map-flush-once") && flush_calls == 1) return VK_ERROR_OUT_OF_HOST_MEMORY;
    NEXT(vkFlushMappedMemoryRanges); return next(device, count, ranges);
}
VKAPI_ATTR VkResult VKAPI_CALL vkInvalidateMappedMemoryRanges(VkDevice device,
    uint32_t count, const VkMappedMemoryRange *ranges) {
    ++invalidate_calls;
    if (count != 1 || ranges[0].sType != VK_STRUCTURE_TYPE_MAPPED_MEMORY_RANGE ||
        ranges[0].pNext || ranges[0].offset != 0 || ranges[0].size != VK_WHOLE_SIZE) abort();
    if (fault("map-invalidate")) return VK_ERROR_OUT_OF_HOST_MEMORY;
    NEXT(vkInvalidateMappedMemoryRanges); return next(device, count, ranges);
}

static int copy_once(const char *mode) {
    if (!fault(mode) || injected_copy_fault) return 0;
    injected_copy_fault = 1; return 1;
}
static int command_index(VkCommandBuffer command) {
    for (int i = 0; i < 256; ++i) if (command_ids[i] == (uint64_t)(uintptr_t)command) return i;
    abort();
}
VKAPI_ATTR VkResult VKAPI_CALL vkCreateCommandPool(VkDevice device,
    const VkCommandPoolCreateInfo *info, const VkAllocationCallbacks *allocator, VkCommandPool *pool) {
    ++pool_creates;
    if (info->sType != VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO || info->pNext ||
        info->flags != VK_COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BIT) abort();
    if (fault("copy-pool-status")) return VK_ERROR_OUT_OF_HOST_MEMORY;
    if (fault("copy-pool-empty")) { *pool = VK_NULL_HANDLE; return VK_SUCCESS; }
    NEXT(vkCreateCommandPool); VkResult status = next(device, info, allocator, pool);
    if (status == VK_SUCCESS) track(copy_pools, (uint64_t)(uintptr_t)*pool, 1);
    return status;
}
VKAPI_ATTR void VKAPI_CALL vkDestroyCommandPool(VkDevice device, VkCommandPool pool,
    const VkAllocationCallbacks *allocator) {
    ++pool_destroys;
    for (int i = 0; i < 256; ++i) if (command_pools[i] == (uint64_t)(uintptr_t)pool) {
        if (command_signals[i]) { fputs("FAIL: pending native command destroyed\n", stderr); abort(); }
        command_ids[i] = command_pools[i] = command_semaphores[i] = 0;
    }
    track(copy_pools, (uint64_t)(uintptr_t)pool, 0);
    NEXT(vkDestroyCommandPool); next(device, pool, allocator);
}
VKAPI_ATTR VkResult VKAPI_CALL vkCreateSemaphore(VkDevice device,
    const VkSemaphoreCreateInfo *info, const VkAllocationCallbacks *allocator, VkSemaphore *semaphore) {
    ++semaphore_creates;
    const VkSemaphoreTypeCreateInfo *timeline = info->pNext;
    if (info->sType != VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO || info->flags || !timeline ||
        timeline->sType != VK_STRUCTURE_TYPE_SEMAPHORE_TYPE_CREATE_INFO || timeline->pNext ||
        timeline->semaphoreType != VK_SEMAPHORE_TYPE_TIMELINE || timeline->initialValue) abort();
    if (fault("copy-semaphore-status")) return VK_ERROR_OUT_OF_DEVICE_MEMORY;
    if (fault("copy-semaphore-empty")) { *semaphore = VK_NULL_HANDLE; return VK_SUCCESS; }
    NEXT(vkCreateSemaphore); VkResult status = next(device, info, allocator, semaphore);
    if (status == VK_SUCCESS) track(copy_semaphores, (uint64_t)(uintptr_t)*semaphore, 1);
    return status;
}
VKAPI_ATTR void VKAPI_CALL vkDestroySemaphore(VkDevice device, VkSemaphore semaphore,
    const VkAllocationCallbacks *allocator) {
    ++semaphore_destroys; track(copy_semaphores, (uint64_t)(uintptr_t)semaphore, 0);
    NEXT(vkDestroySemaphore); next(device, semaphore, allocator);
}
VKAPI_ATTR VkResult VKAPI_CALL vkAllocateCommandBuffers(VkDevice device,
    const VkCommandBufferAllocateInfo *info, VkCommandBuffer *commands) {
    ++command_allocations;
    if (info->sType != VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO || info->pNext ||
        info->level != VK_COMMAND_BUFFER_LEVEL_PRIMARY || !info->commandBufferCount || info->commandBufferCount > 256) abort();
    if (fault("copy-commands-status")) return VK_ERROR_OUT_OF_HOST_MEMORY;
    NEXT(vkAllocateCommandBuffers); VkResult status = next(device, info, commands);
    if (status != VK_SUCCESS) return status;
    for (uint32_t j = 0; j < info->commandBufferCount; ++j) {
        int found = 0;
        for (int i = 0; i < 256; ++i) if (!command_ids[i]) {
            command_ids[i] = (uint64_t)(uintptr_t)commands[j];
            command_pools[i] = (uint64_t)(uintptr_t)info->commandPool;
            command_semaphores[i] = command_signals[i] = 0; ++command_count; found = 1; break;
        }
        if (!found) abort();
    }
    if (fault("copy-command-empty")) commands[0] = VK_NULL_HANDLE;
    if (fault("copy-command-duplicate") && info->commandBufferCount > 1) commands[1] = commands[0];
    return status;
}
VKAPI_ATTR VkResult VKAPI_CALL vkResetCommandBuffer(VkCommandBuffer command, VkCommandBufferResetFlags flags) {
    if (command_signals[command_index(command)] || flags) abort();
    if (fault("copy-reset-status")) return VK_ERROR_OUT_OF_HOST_MEMORY;
    NEXT(vkResetCommandBuffer); return next(command, flags);
}
VKAPI_ATTR VkResult VKAPI_CALL vkBeginCommandBuffer(VkCommandBuffer command, const VkCommandBufferBeginInfo *info) {
    if (info->sType != VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO || info->pNext ||
        info->flags != VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT || info->pInheritanceInfo) abort();
    if (fault("copy-begin-status")) return VK_ERROR_OUT_OF_DEVICE_MEMORY;
    NEXT(vkBeginCommandBuffer); return next(command, info);
}
VKAPI_ATTR VkResult VKAPI_CALL vkEndCommandBuffer(VkCommandBuffer command) {
    if (fault("copy-end-status")) return VK_ERROR_OUT_OF_HOST_MEMORY;
    NEXT(vkEndCommandBuffer); VkResult result = next(command);
    if (result == VK_SUCCESS && fault("copy-late-timeout")) copy_clock_late = 1;
    return result;
}
VKAPI_ATTR void VKAPI_CALL vkCmdCopyBuffer(VkCommandBuffer command, VkBuffer source,
    VkBuffer destination, uint32_t count, const VkBufferCopy *regions) {
    ++copy_calls;
    if (count != 1 || !regions->size || command_signals[command_index(command)]) abort();
    NEXT(vkCmdCopyBuffer); next(command, source, destination, count, regions);
}
VKAPI_ATTR void VKAPI_CALL vkCmdPipelineBarrier(VkCommandBuffer command, VkPipelineStageFlags source,
    VkPipelineStageFlags destination, VkDependencyFlags flags, uint32_t memory_count,
    const VkMemoryBarrier *memory, uint32_t buffer_count, const VkBufferMemoryBarrier *buffers,
    uint32_t image_count, const VkImageMemoryBarrier *images) {
    if (flags || memory_count || memory || image_count || images || buffer_count != 1 ||
        buffers->sType != VK_STRUCTURE_TYPE_BUFFER_MEMORY_BARRIER || buffers->pNext || !buffers->size ||
        buffers->srcQueueFamilyIndex != VK_QUEUE_FAMILY_IGNORED || buffers->dstQueueFamilyIndex != VK_QUEUE_FAMILY_IGNORED) abort();
    if (!((source == (VK_PIPELINE_STAGE_ALL_COMMANDS_BIT | VK_PIPELINE_STAGE_HOST_BIT) && destination == VK_PIPELINE_STAGE_TRANSFER_BIT) ||
        (source == VK_PIPELINE_STAGE_TRANSFER_BIT && destination == (VK_PIPELINE_STAGE_ALL_COMMANDS_BIT | VK_PIPELINE_STAGE_HOST_BIT)))) abort();
    NEXT(vkCmdPipelineBarrier); next(command, source, destination, flags, memory_count, memory,
        buffer_count, buffers, image_count, images);
}
VKAPI_ATTR VkResult VKAPI_CALL vkQueueSubmit(VkQueue queue, uint32_t count, const VkSubmitInfo *infos, VkFence fence) {
    ++submit_calls;
    const VkTimelineSemaphoreSubmitInfo *timeline = infos->pNext;
    if (count != 1 || fence || infos->sType != VK_STRUCTURE_TYPE_SUBMIT_INFO || !timeline ||
        timeline->sType != VK_STRUCTURE_TYPE_TIMELINE_SEMAPHORE_SUBMIT_INFO || timeline->pNext ||
        timeline->waitSemaphoreValueCount || timeline->pWaitSemaphoreValues || timeline->signalSemaphoreValueCount != 1 ||
        !timeline->pSignalSemaphoreValues || !timeline->pSignalSemaphoreValues[0] || infos->waitSemaphoreCount ||
        infos->pWaitSemaphores || infos->pWaitDstStageMask || infos->commandBufferCount != 1 || infos->signalSemaphoreCount != 1) abort();
    int index = command_index(infos->pCommandBuffers[0]);
    if (command_signals[index]) abort();
    if (copy_once("copy-submit-oom")) return VK_ERROR_OUT_OF_HOST_MEMORY;
    NEXT(vkQueueSubmit); VkResult status = next(queue, count, infos, fence);
    if (status == VK_SUCCESS) {
        command_semaphores[index] = (uint64_t)(uintptr_t)infos->pSignalSemaphores[0];
        command_signals[index] = timeline->pSignalSemaphoreValues[0];
        if (fault("copy-post-submit-timeout")) copy_clock_late = 1;
        if (fault("copy-submit-lost")) return VK_ERROR_DEVICE_LOST;
        if (fault("copy-submit-unknown")) return VK_ERROR_UNKNOWN;
    }
    return status;
}
VKAPI_ATTR VkResult VKAPI_CALL vkWaitSemaphores(VkDevice device, const VkSemaphoreWaitInfo *info, uint64_t timeout) {
    ++poll_calls;
    if (timeout || info->sType != VK_STRUCTURE_TYPE_SEMAPHORE_WAIT_INFO || info->pNext || info->flags ||
        info->semaphoreCount != 1 || !info->pSemaphores || !info->pValues || !info->pValues[0]) abort();
    if (copy_once("copy-poll-oom")) return VK_ERROR_OUT_OF_DEVICE_MEMORY;
    if (copy_once("copy-poll-timeout")) return VK_TIMEOUT;
    NEXT(vkWaitSemaphores); VkResult status = next(device, info, timeout);
    if (status == VK_SUCCESS || status == VK_ERROR_DEVICE_LOST) {
        for (int i = 0; i < 256; ++i) if (command_semaphores[i] == (uint64_t)(uintptr_t)info->pSemaphores[0] && command_signals[i] <= info->pValues[0])
            command_signals[i] = 0;
        if (fault("copy-poll-lost")) return VK_ERROR_DEVICE_LOST;
    }
    return status;
}
VKAPI_ATTR VkResult VKAPI_CALL vkQueueWaitIdle(VkQueue queue) {
    if (getenv("SEEN_VULKAN_FIXTURE_COPY_TRACE")) { fputs("FAIL: copy used queue-wide idle wait\n", stderr); abort(); }
    NEXT(vkQueueWaitIdle); return next(queue);
}
VKAPI_ATTR VkResult VKAPI_CALL vkDeviceWaitIdle(VkDevice device) {
    if (getenv("SEEN_VULKAN_FIXTURE_COPY_TRACE")) { fputs("FAIL: copy used device-wide idle wait\n", stderr); abort(); }
    NEXT(vkDeviceWaitIdle); return next(device);
}
VKAPI_ATTR VkResult VKAPI_CALL vkCreateFence(VkDevice device, const VkFenceCreateInfo *info,
    const VkAllocationCallbacks *allocator, VkFence *fence) {
    if (getenv("SEEN_VULKAN_FIXTURE_COPY_TRACE")) { fputs("FAIL: copy created a per-operation fence\n", stderr); abort(); }
    NEXT(vkCreateFence); return next(device, info, allocator, fence);
}

int clock_gettime(clockid_t clock, struct timespec *value) {
    /* Sanitizer timing can reenter loader error cleanup. Do not dlsym from
     * this hook; the test-only Linux clock path preserves the kernel result. */
    int status = (int)syscall(SYS_clock_gettime, clock, value);
    if (!status && clock == CLOCK_MONOTONIC && copy_clock_late) value->tv_sec += 2;
    return status;
}
