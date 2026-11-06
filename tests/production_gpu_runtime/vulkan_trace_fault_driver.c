/* Test-only Runtime H observability. All GPU work and resource policy stay in
 * Seen; this unit forwards the installed SDK ABI and rejects forbidden calls. */
#define _GNU_SOURCE
#include <stddef.h>
#include <stdint.h>
#define SEEN_VULKAN_TRACE_FIXTURE 1
static void h_acquisition(const void *family);
static void h_descriptor_allocation(uint32_t count);
static void h_cache_initial(size_t bytes);
static void h_submitted(int index);
static void h_polled(void);
static void h_forbidden(const char *operation);
static void h_memory_allocated(uint64_t id, uint64_t bytes);
static void h_memory_freed(uint64_t id);
#include "vulkan_recovery_fault_driver.c"
#include <inttypes.h>
#include <sys/resource.h>

/* These identities are the existing drivers' actual bounded ownership tables,
 * not a second handle registry or a synthetic resource-success model. */
static struct {
    const char *name;
    const uint64_t *ids;
    size_t capacity;
    uint64_t acquisitions;
} h_families[] = {
    {"buffer", buffer_ids, 64, 0},
    {"device_memory", memory_ids, 64, 0},
    {"mapped_memory", mapped_ids, 64, 0},
    {"shader_module", shader_ids, 64, 0},
    {"descriptor_set_layout", set_layout_ids, 64, 0},
    {"pipeline_layout", pipeline_layout_ids, 64, 0},
    {"pipeline_cache", cache_ids, 64, 0},
    {"pipeline", pipeline_ids, 64, 0},
    {"descriptor_pool", descriptor_pool_ids, 64, 0},
    {"command_pool", copy_pools, 64, 0},
    {"timeline_semaphore", copy_semaphores, 64, 0},
    {"command_buffer", command_ids, 256, 0},
    {"query_pool", query_ids, 64, 0}
};
static uint64_t h_acquisitions, h_hot_acquisitions;
static uint64_t h_descriptor_calls, h_descriptor_sets, h_hot_descriptor_sets;
static uint64_t h_cache_calls, h_nonempty_cache_calls, h_cache_initial_bytes;
static uint64_t h_hot_submissions, h_hot_polls;
static uint64_t h_hot_command, h_hot_command_recordings;
static uint64_t h_forbidden_calls, h_fence_calls, h_global_wait_calls;
static struct { uint64_t id, bytes; } h_memory[64];
static uint64_t h_memory_live_bytes, h_memory_peak_bytes, h_memory_total_bytes;

static int h_hot(void) {
    const char *phase = getenv("SEEN_VULKAN_FIXTURE_TRACE_PHASE");
    return phase && strcmp(phase, "hot") == 0;
}
static void h_failure(const char *operation) {
    fprintf(stderr, "GPU_RUNTIME_H_VIOLATION {\"kind\":\"audit_failure\",\"operation\":\"%s\",\"exit_code\":97}\n", operation);
    _Exit(97);
}
static void h_add(uint64_t *counter, uint64_t amount) {
    if (UINT64_MAX - *counter < amount) h_failure("counter_overflow");
    *counter += amount;
}
static void h_forbidden(const char *operation) {
    /* LD_PRELOAD is inherited by shell/loader helper processes. Only the actual
     * Seen fixture sets this in-process marker; helpers must still forward. */
    if (!getenv("SEEN_VULKAN_FIXTURE_TRACE_PHASE")) return;
    h_add(&h_forbidden_calls, 1);
    if (strcmp(operation, "vkCreateFence") == 0 ||
        strcmp(operation, "vkWaitForFences") == 0 ||
        strcmp(operation, "vkResetFences") == 0) h_add(&h_fence_calls, 1);
    if (strcmp(operation, "vkQueueWaitIdle") == 0 ||
        strcmp(operation, "vkDeviceWaitIdle") == 0) h_add(&h_global_wait_calls, 1);
    fprintf(stderr, "GPU_RUNTIME_H_VIOLATION {\"kind\":\"forbidden_native_call\",\"operation\":\"%s\",\"hot\":%s,\"exit_code\":90}\n",
        operation, h_hot() ? "true" : "false");
    _Exit(90);
}
static void h_acquisition(const void *family) {
    for (size_t i = 0; i < sizeof h_families / sizeof h_families[0]; ++i) {
        if (family != h_families[i].ids) continue;
        h_add(&h_families[i].acquisitions, 1);
        h_add(&h_acquisitions, 1);
        if (h_hot()) {
            h_add(&h_hot_acquisitions, 1);
            h_forbidden(h_families[i].name);
        }
        return;
    }
    h_failure("unknown_resource_family");
}
static void h_descriptor_allocation(uint32_t count) {
    h_add(&h_descriptor_calls, 1);
    h_add(&h_descriptor_sets, count);
    if (h_hot()) {
        h_add(&h_hot_descriptor_sets, count);
        h_forbidden("vkAllocateDescriptorSets");
    }
}
static void h_cache_initial(size_t bytes) {
    h_add(&h_cache_calls, 1);
    if (bytes) {
        h_add(&h_nonempty_cache_calls, 1);
        h_add(&h_cache_initial_bytes, bytes);
    }
    if (h_hot()) h_forbidden("vkCreatePipelineCache");
}
static void h_submitted(int index) {
    if (!h_hot()) return;
    if (index < 0 || index >= 256 || !command_ids[index] ||
        !graph_reusable[index] || graph_records[index] != 1 ||
        graph_dispatches[index] != 1 || graph_submits[index] < 5)
        h_failure("hot_command_not_one_recorded_warmed_graph");
    if (h_hot_command && h_hot_command != command_ids[index])
        h_failure("hot_command_identity_changed");
    h_hot_command = command_ids[index];
    h_hot_command_recordings = graph_records[index];
    h_add(&h_hot_submissions, 1);
}
static void h_polled(void) {
    if (h_hot()) h_add(&h_hot_polls, 1);
}
static void h_memory_allocated(uint64_t id, uint64_t bytes) {
    if (!id || !bytes) h_failure("invalid_memory_allocation");
    size_t available = sizeof h_memory / sizeof h_memory[0];
    for (size_t i = 0; i < sizeof h_memory / sizeof h_memory[0]; ++i) {
        if (h_memory[i].id == id) h_failure("duplicate_memory_allocation");
        if (!h_memory[i].id) available = i;
    }
    if (available == sizeof h_memory / sizeof h_memory[0]) h_failure("memory_observation_capacity");
    h_memory[available].id = id;
    h_memory[available].bytes = bytes;
    h_add(&h_memory_live_bytes, bytes);
    h_add(&h_memory_total_bytes, bytes);
    if (h_memory_live_bytes > h_memory_peak_bytes) h_memory_peak_bytes = h_memory_live_bytes;
}
static void h_memory_freed(uint64_t id) {
    for (size_t i = 0; i < sizeof h_memory / sizeof h_memory[0]; ++i) {
        if (!id || h_memory[i].id != id) continue;
        if (h_memory_live_bytes < h_memory[i].bytes) h_failure("memory_byte_underflow");
        h_memory_live_bytes -= h_memory[i].bytes;
        h_memory[i].id = h_memory[i].bytes = 0;
        return;
    }
    h_failure("untracked_memory_free");
}

/* SDK types, calling convention, and PFN declarations are supplied by the real
 * installed Vulkan headers included by the common driver. vkCreateFence and
 * both global-idle wrappers are already defined there and use h_forbidden. */
VKAPI_ATTR VkResult VKAPI_CALL vkWaitForFences(VkDevice device, uint32_t count,
    const VkFence *fences, VkBool32 wait_all, uint64_t timeout) {
    h_forbidden("vkWaitForFences");
    NEXT(vkWaitForFences); return next(device, count, fences, wait_all, timeout);
}
VKAPI_ATTR VkResult VKAPI_CALL vkResetFences(VkDevice device, uint32_t count,
    const VkFence *fences) {
    h_forbidden("vkResetFences");
    NEXT(vkResetFences); return next(device, count, fences);
}

__attribute__((destructor)) static void require_h_trace_drain(void) {
    const char *phase = getenv("SEEN_VULKAN_FIXTURE_TRACE_PHASE");
    if (!phase) return;
    if (strcmp(phase, "done") != 0) h_failure("phase_not_done");
    for (size_t i = 0; i < sizeof h_families / sizeof h_families[0]; ++i)
        for (size_t j = 0; j < h_families[i].capacity; ++j)
            if (h_families[i].ids[j]) h_failure(h_families[i].name);
    for (size_t i = 0; i < sizeof h_memory / sizeof h_memory[0]; ++i)
        if (h_memory[i].id || h_memory[i].bytes) h_failure("memory_observation_not_drained");
    for (size_t i = 0; i < 256; ++i)
        if (command_pools[i] || command_semaphores[i] || command_signals[i]) h_failure("command_not_drained");
    if (h_memory_live_bytes) h_failure("memory_bytes_not_drained");
    if (h_hot_acquisitions || h_hot_descriptor_sets || h_forbidden_calls) h_failure("forbidden_call_survived");
    if (h_hot_submissions < 30 || h_hot_polls < 30) h_failure("insufficient_hot_native_work");
    if (h_cache_calls < 2 || !h_nonempty_cache_calls || !h_cache_initial_bytes)
        h_failure("native_cache_roundtrip_not_observed");
    if (!h_memory_peak_bytes || !h_descriptor_calls || !query_creates ||
        !h_hot_command || h_hot_command_recordings != 1)
        h_failure("persistent_native_setup_not_observed");

    struct rusage usage;
    int have_rss = getrusage(RUSAGE_SELF, &usage) == 0 && usage.ru_maxrss >= 0;
    fprintf(stderr, "GPU_RUNTIME_H_TRACE {\"schema_version\":1,\"drained\":true,\"native_acquisitions\":%" PRIu64 ",\"acquisitions_by_family\":{", h_acquisitions);
    for (size_t i = 0; i < sizeof h_families / sizeof h_families[0]; ++i)
        fprintf(stderr, "%s\"%s\":%" PRIu64, i ? "," : "", h_families[i].name, h_families[i].acquisitions);
    fprintf(stderr, "},\"native_live_objects\":0,\"hot_acquisitions\":%" PRIu64
        ",\"descriptor_allocation_calls\":%" PRIu64 ",\"descriptor_sets_attempted\":%" PRIu64
        ",\"hot_descriptor_sets_attempted\":%" PRIu64 ",\"pipeline_cache_create_calls\":%" PRIu64
        ",\"pipeline_cache_nonempty_initial_calls\":%" PRIu64 ",\"pipeline_cache_initial_bytes\":%" PRIu64
        ",\"hot_submissions\":%" PRIu64 ",\"hot_polls\":%" PRIu64
        ",\"hot_command_recordings\":%" PRIu64
        ",\"forbidden_calls\":%" PRIu64 ",\"forbidden_fence_calls\":%" PRIu64
        ",\"forbidden_global_wait_calls\":%" PRIu64 ",\"requested_device_memory_total_bytes\":%" PRIu64
        ",\"requested_device_memory_peak_bytes\":%" PRIu64 ",\"requested_device_memory_live_bytes\":%" PRIu64
        ",\"physical_vram_bytes\":null,\"physical_vram_status\":\"not_measured\",\"timestamp_query_read_calls\":%u"
        ",\"timestamp_query_result_requested_bytes\":%" PRIu64 ",\"self_peak_rss_kib\":",
        h_hot_acquisitions, h_descriptor_calls, h_descriptor_sets, h_hot_descriptor_sets,
        h_cache_calls, h_nonempty_cache_calls, h_cache_initial_bytes, h_hot_submissions, h_hot_polls,
        h_hot_command_recordings,
        h_forbidden_calls, h_fence_calls, h_global_wait_calls, h_memory_total_bytes,
        h_memory_peak_bytes, h_memory_live_bytes, query_reads, (uint64_t)query_reads * 32);
    if (have_rss) fprintf(stderr, "%ld", usage.ru_maxrss);
    else fputs("null", stderr);
    fprintf(stderr, ",\"self_peak_rss_status\":\"%s\"}\n", have_rss ? "getrusage_self_linux" : "unavailable");
}
