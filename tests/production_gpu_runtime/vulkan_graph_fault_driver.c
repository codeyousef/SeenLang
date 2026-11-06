/* Test-only direct SDK forwarding and bounded fault injection for Runtime F.
 * The common driver retains its stricter ONE_TIME policy outside this unit. */
#define SEEN_VULKAN_GRAPH_FIXTURE 1
static unsigned graph_records[256], graph_submits[256], graph_dispatches[256], graph_reusable[256];
static unsigned graph_reuses, graph_dispatch_total;
#include "vulkan_fault_driver.c"

static uint64_t query_ids[64], query_commands[64];
static unsigned query_creates, query_destroys, query_reads, query_resets, query_writes;
static int query_index(VkQueryPool pool) {
    for (int i = 0; i < 64; ++i)
        if (pool && query_ids[i] == (uint64_t)(uintptr_t)pool) return i;
    abort();
}
VKAPI_ATTR VkResult VKAPI_CALL vkCreateQueryPool(VkDevice device,
    const VkQueryPoolCreateInfo *info, const VkAllocationCallbacks *allocator, VkQueryPool *pool) {
    if (!device || !info || !pool || info->sType != VK_STRUCTURE_TYPE_QUERY_POOL_CREATE_INFO ||
        info->pNext || info->flags || info->queryType != VK_QUERY_TYPE_TIMESTAMP ||
        info->queryCount != 2 || info->pipelineStatistics) abort();
    NEXT(vkCreateQueryPool); VkResult result = next(device, info, allocator, pool);
    if (result == VK_SUCCESS && *pool) {
        ++query_creates; track(query_ids, (uint64_t)(uintptr_t)*pool, 1);
        query_commands[query_index(*pool)] = 0;
        if (fault("timestamp-create-null")) {
            PFN_vkDestroyQueryPool destroy = (PFN_vkDestroyQueryPool)dlsym(RTLD_NEXT, "vkDestroyQueryPool");
            if (!destroy) abort();
            destroy(device, *pool, allocator); track(query_ids, (uint64_t)(uintptr_t)*pool, 0);
            ++query_destroys; *pool = VK_NULL_HANDLE; return VK_SUCCESS;
        }
        if (fault("timestamp-create-failed-output")) return VK_ERROR_OUT_OF_HOST_MEMORY;
    }
    return result;
}
VKAPI_ATTR void VKAPI_CALL vkDestroyQueryPool(VkDevice device, VkQueryPool pool,
    const VkAllocationCallbacks *allocator) {
    int slot = query_index(pool);
    for (int i = 0; i < 256; ++i)
        if (command_ids[i] == query_commands[slot] && command_signals[i]) abort();
    query_commands[slot] = 0; track(query_ids, (uint64_t)(uintptr_t)pool, 0); ++query_destroys;
    NEXT(vkDestroyQueryPool); next(device, pool, allocator);
}
VKAPI_ATTR void VKAPI_CALL vkCmdResetQueryPool(VkCommandBuffer command, VkQueryPool pool,
    uint32_t first, uint32_t count) {
    if (first || count != 2 || command_signals[command_index(command)]) abort();
    query_commands[query_index(pool)] = (uint64_t)(uintptr_t)command; ++query_resets;
    NEXT(vkCmdResetQueryPool); next(command, pool, first, count);
}
VKAPI_ATTR void VKAPI_CALL vkCmdWriteTimestamp(VkCommandBuffer command, VkPipelineStageFlagBits stage,
    VkQueryPool pool, uint32_t query) {
    if (query > 1 || (query == 0 && stage != VK_PIPELINE_STAGE_TOP_OF_PIPE_BIT) ||
        (query == 1 && stage != VK_PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT) ||
        command_signals[command_index(command)] || query_commands[query_index(pool)] != (uint64_t)(uintptr_t)command) abort();
    ++query_writes; NEXT(vkCmdWriteTimestamp); next(command, stage, pool, query);
}
VKAPI_ATTR VkResult VKAPI_CALL vkGetQueryPoolResults(VkDevice device, VkQueryPool pool,
    uint32_t first, uint32_t count, size_t bytes, void *data, VkDeviceSize stride, VkQueryResultFlags flags) {
    if (!device || first || count != 2 || bytes != 32 || !data || stride != 16 ||
        flags != (VK_QUERY_RESULT_64_BIT | VK_QUERY_RESULT_WITH_AVAILABILITY_BIT)) abort();
    (void)query_index(pool); ++query_reads;
    NEXT(vkGetQueryPoolResults); VkResult result = next(device, pool, first, count, bytes, data, stride, flags);
    if (fault("timestamp-unavailable")) return VK_NOT_READY;
    if (fault("timestamp-zero-availability")) {
        ((uint64_t *)data)[1] = ((uint64_t *)data)[3] = 0; return VK_SUCCESS;
    }
    if (fault("timestamp-query-failed")) return VK_ERROR_UNKNOWN;
    if (fault("timestamp-query-lost")) return VK_ERROR_DEVICE_LOST;
    return result;
}
VKAPI_ATTR void VKAPI_CALL vkCmdDispatch(VkCommandBuffer command, uint32_t x, uint32_t y, uint32_t z) {
    int index = command_index(command);
    if (!x || !y || !z || command_signals[index] || graph_records[index] != 1) abort();
    ++graph_dispatches[index]; ++graph_dispatch_total;
    NEXT(vkCmdDispatch); next(command, x, y, z);
}
__attribute__((destructor)) static void require_graph_drain(void) {
    if (!query_creates && !pool_creates) return;
    for (int i = 0; i < 64; ++i) if (query_ids[i]) {
        fputs("FAIL: timestamp fixture retained native query ownership\n", stderr); _Exit(97);
    }
    if (getenv("SEEN_VULKAN_FIXTURE_GRAPH_REUSE") && !graph_reuses) {
        fputs("FAIL: native graph never reused one recorded multi-kernel batch\n", stderr); _Exit(97);
    }
    fprintf(stderr, "VULKAN_GRAPH_TRACE reusable=%u recorded-kernels=%u drained=1\n", graph_reuses, graph_dispatch_total);
    fprintf(stderr, "VULKAN_TIMESTAMP_TRACE pools=%u/%u reset=%u writes=%u reads=%u faults=%llu drained=1\n",
        query_creates, query_destroys, query_resets, query_writes, query_reads, (unsigned long long)fault_hits);
}
