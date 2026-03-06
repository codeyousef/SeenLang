// Seen Region Runtime Implementation
// Provides region-based memory allocation for @preallocate decorator
// Supports CXL-aware NUMA-affinity allocation via mbind()

#ifdef __linux__
#define _GNU_SOURCE
#endif
#include "seen_region.h"
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <sys/mman.h>

#ifdef __linux__
#include <unistd.h>
#include <linux/mempolicy.h>
#include <sys/syscall.h>

// Wrapper for mbind syscall (avoids libnuma dependency)
static long seen_mbind(void *addr, unsigned long len, int mode,
                       const unsigned long *nodemask, unsigned long maxnode,
                       unsigned flags) {
    return syscall(SYS_mbind, addr, len, mode, nodemask, maxnode, flags);
}

static int seen_numa_node_count(void) {
    // Count NUMA nodes via /sys
    int count = 0;
    char path[128];
    for (int i = 0; i < 16; i++) {
        snprintf(path, sizeof(path), "/sys/devices/system/node/node%d", i);
        if (access(path, F_OK) == 0) {
            count++;
        } else {
            break;
        }
    }
    return count;
}
#endif

// ============================================================================
// Configuration
// ============================================================================

#define SEEN_MAX_REGIONS 256
#define SEEN_MAX_REGION_STACK 64
#define SEEN_MAX_HANDLES 65536
#define SEEN_DEFAULT_ALIGNMENT 16
#define SEEN_GLOBAL_REGION_SIZE (64 * 1024 * 1024)  // 64MB default global region

// ============================================================================
// Global State
// ============================================================================

// Active regions
static SeenRegion* g_regions[SEEN_MAX_REGIONS];
static int g_region_count = 0;
static int64_t g_next_region_id = 1;

// Region stack for nested allocations
static SeenRegion* g_region_stack[SEEN_MAX_REGION_STACK];
static int g_region_stack_depth = 0;

// Global default region
static SeenRegion* g_global_region = NULL;

// Generational handle table
typedef struct {
    void* ptr;              // Pointer to allocated memory
    int64_t generation;     // Generation counter
    int64_t region_id;      // Region this belongs to
    int active;             // Is this slot active?
} HandleSlot;

static HandleSlot g_handle_table[SEEN_MAX_HANDLES];
static int64_t g_next_handle_slot = 0;

// Initialization flag
static int g_region_initialized = 0;

// ============================================================================
// Initialization
// ============================================================================

void __seen_region_init(void) {
    if (g_region_initialized) return;
    g_region_initialized = 1;

    // Initialize handle table
    memset(g_handle_table, 0, sizeof(g_handle_table));

    // Create global default region
    g_global_region = __seen_region_create_named(
        SEEN_GLOBAL_REGION_SIZE,
        SEEN_REGION_BUMP,
        "global"
    );
}

// ============================================================================
// Region Creation/Destruction
// ============================================================================

SeenRegion* __seen_region_create(int64_t size, int32_t strategy) {
    return __seen_region_create_named(size, strategy, NULL);
}

SeenRegion* __seen_region_create_named(int64_t size, int32_t strategy, const char* name) {
    __seen_region_init();

    if (g_region_count >= SEEN_MAX_REGIONS) {
        fprintf(stderr, "ERROR: Maximum number of regions reached (%d)\n", SEEN_MAX_REGIONS);
        return NULL;
    }

    if (size <= 0) {
        fprintf(stderr, "ERROR: Invalid region size: %ld\n", (long)size);
        return NULL;
    }

    // Allocate region structure
    SeenRegion* region = (SeenRegion*)malloc(sizeof(SeenRegion));
    if (!region) {
        fprintf(stderr, "ERROR: Failed to allocate region structure\n");
        return NULL;
    }

    // Allocate region memory (strategy-dependent)
    void* base = NULL;

    if (strategy == SEEN_REGION_GPU) {
        // GPU-visible memory: page-aligned mmap suitable for Vulkan vkMapMemory
        base = mmap(NULL, (size_t)size,
                    PROT_READ | PROT_WRITE,
                    MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
        if (base == MAP_FAILED) {
            fprintf(stderr, "ERROR: mmap failed for GPU region (%ld bytes)\n", (long)size);
            free(region);
            return NULL;
        }
    } else if (strategy == SEEN_REGION_CXL_NEAR || strategy == SEEN_REGION_CXL_FAR) {
        // Use mmap for CXL/NUMA-aware allocation
        base = mmap(NULL, (size_t)size,
                    PROT_READ | PROT_WRITE,
                    MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
        if (base == MAP_FAILED) {
            fprintf(stderr, "ERROR: mmap failed for CXL region (%ld bytes)\n", (long)size);
            free(region);
            return NULL;
        }
#ifdef __linux__
        int numa_nodes = seen_numa_node_count();
        if (numa_nodes > 1) {
            if (strategy == SEEN_REGION_CXL_NEAR) {
                // Bind to local NUMA node (node 0)
                unsigned long nodemask = 1UL;
                seen_mbind(base, (size_t)size, MPOL_BIND, &nodemask, 8, 0);
            } else {
                // Bind to remote/CXL NUMA node (node 1+)
                unsigned long nodemask = 2UL;
                seen_mbind(base, (size_t)size, MPOL_BIND, &nodemask, 8, 0);
                // Falls back to default allocation if mbind fails
            }
        }
#endif
    } else {
        base = malloc((size_t)size);
    }

    if (!base) {
        fprintf(stderr, "ERROR: Failed to allocate region memory (%ld bytes)\n", (long)size);
        free(region);
        return NULL;
    }

    // Initialize region
    region->base = base;
    region->size = (size_t)size;
    region->offset = 0;
    region->peak_usage = 0;
    region->alloc_count = 0;
    region->budget = 0;
    region->region_id = g_next_region_id++;
    region->parent_id = (g_region_stack_depth > 0)
        ? g_region_stack[g_region_stack_depth - 1]->region_id
        : -1;
    region->strategy = (SeenRegionStrategy)strategy;
    region->active = 1;
    region->name = name;
    region->pinning_attrs = NULL;  // VSD pinning attributes (optional)

    // Register region
    g_regions[g_region_count++] = region;

    return region;
}

void __seen_region_destroy(SeenRegion* region) {
    if (!region) return;

    // Cascading destroy: destroy child regions first
    for (int i = g_region_count - 1; i >= 0; i--) {
        if (g_regions[i] && g_regions[i]->parent_id == region->region_id) {
            __seen_region_destroy(g_regions[i]);
            i = g_region_count;  // restart scan since array shifted
        }
    }

    // Invalidate all handles for this region
    for (int i = 0; i < SEEN_MAX_HANDLES; i++) {
        if (g_handle_table[i].region_id == region->region_id && g_handle_table[i].active) {
            g_handle_table[i].active = 0;
            g_handle_table[i].generation++;
            g_handle_table[i].ptr = NULL;
        }
    }

    // Free pinning attributes if present
    if (region->pinning_attrs) {
        free(region->pinning_attrs);
        region->pinning_attrs = NULL;
    }

    // Free memory (strategy-dependent)
    if (region->base) {
        if (region->strategy == SEEN_REGION_GPU ||
            region->strategy == SEEN_REGION_CXL_NEAR ||
            region->strategy == SEEN_REGION_CXL_FAR) {
            munmap(region->base, region->size);
        } else {
            free(region->base);
        }
        region->base = NULL;
    }

    region->active = 0;
    region->size = 0;
    region->offset = 0;

    // Remove from regions list
    for (int i = 0; i < g_region_count; i++) {
        if (g_regions[i] == region) {
            // Shift remaining regions
            for (int j = i; j < g_region_count - 1; j++) {
                g_regions[j] = g_regions[j + 1];
            }
            g_region_count--;
            break;
        }
    }

    // Free region structure
    free(region);
}

void __seen_region_reset(SeenRegion* region) {
    if (!region) return;

    // Reset offset but keep memory
    region->offset = 0;
    region->alloc_count = 0;

    // Invalidate handles but don't free
    for (int i = 0; i < SEEN_MAX_HANDLES; i++) {
        if (g_handle_table[i].region_id == region->region_id && g_handle_table[i].active) {
            g_handle_table[i].active = 0;
            g_handle_table[i].generation++;
        }
    }
}

// ============================================================================
// Region Allocation
// ============================================================================

void* __seen_region_alloc(SeenRegion* region, int64_t size, int64_t alignment) {
    if (!region || !region->active || !region->base) {
        return NULL;
    }

    if (size <= 0) {
        return NULL;
    }

    // Use default alignment if not specified
    if (alignment <= 0) {
        alignment = SEEN_DEFAULT_ALIGNMENT;
    }

    // Align current offset
    size_t aligned_offset = (region->offset + alignment - 1) & ~(alignment - 1);

    // Check budget
    if (region->budget > 0 && aligned_offset + (size_t)size > region->budget) {
        fprintf(stderr, "ERROR: Region #%ld budget exceeded (%zu / %zu bytes)\n",
                (long)region->region_id, aligned_offset + (size_t)size, region->budget);
        return NULL;
    }

    // Check if there's enough space
    if (aligned_offset + (size_t)size > region->size) {
        // Region is full
        return NULL;
    }

    // Get pointer to allocated memory
    void* ptr = (char*)region->base + aligned_offset;

    // Update offset
    region->offset = aligned_offset + (size_t)size;

    // Update peak usage
    if (region->offset > region->peak_usage) {
        region->peak_usage = region->offset;
    }

    // Update allocation count
    region->alloc_count++;

    // Zero-initialize memory (like calloc)
    memset(ptr, 0, (size_t)size);

    return ptr;
}

int64_t __seen_region_remaining(SeenRegion* region) {
    if (!region || !region->active) return 0;
    return (int64_t)(region->size - region->offset);
}

int64_t __seen_region_used(SeenRegion* region) {
    if (!region || !region->active) return 0;
    return (int64_t)region->offset;
}

int64_t __seen_region_peak(SeenRegion* region) {
    if (!region || !region->active) return 0;
    return (int64_t)region->peak_usage;
}

int64_t __seen_region_alloc_count(SeenRegion* region) {
    if (!region || !region->active) return 0;
    return (int64_t)region->alloc_count;
}

void __seen_region_set_budget(SeenRegion* region, int64_t budget_bytes) {
    if (!region) return;
    region->budget = (budget_bytes > 0) ? (size_t)budget_bytes : 0;
}

// ============================================================================
// Region Stack
// ============================================================================

void __seen_region_push(SeenRegion* region) {
    if (!region) return;

    if (g_region_stack_depth >= SEEN_MAX_REGION_STACK) {
        fprintf(stderr, "ERROR: Region stack overflow (max depth %d)\n", SEEN_MAX_REGION_STACK);
        return;
    }

    g_region_stack[g_region_stack_depth++] = region;
}

SeenRegion* __seen_region_pop(void) {
    if (g_region_stack_depth <= 0) {
        return NULL;
    }

    return g_region_stack[--g_region_stack_depth];
}

SeenRegion* __seen_region_current(void) {
    if (g_region_stack_depth <= 0) {
        return g_global_region;
    }

    return g_region_stack[g_region_stack_depth - 1];
}

SeenRegion* __seen_region_global(void) {
    __seen_region_init();
    return g_global_region;
}

// ============================================================================
// Generational Handles
// ============================================================================

SeenRegionHandle __seen_region_alloc_handle(SeenRegion* region, int64_t size) {
    SeenRegionHandle handle = {0, 0, -1};

    if (!region || !region->active) {
        return handle;
    }

    // Allocate memory
    void* ptr = __seen_region_alloc(region, size, SEEN_DEFAULT_ALIGNMENT);
    if (!ptr) {
        return handle;
    }

    // Find a free slot in handle table
    int slot = -1;
    for (int i = 0; i < SEEN_MAX_HANDLES; i++) {
        int idx = (g_next_handle_slot + i) % SEEN_MAX_HANDLES;
        if (!g_handle_table[idx].active) {
            slot = idx;
            break;
        }
    }

    if (slot < 0) {
        // Handle table full - try to reuse oldest
        slot = g_next_handle_slot % SEEN_MAX_HANDLES;
    }

    // Set up handle slot
    g_handle_table[slot].ptr = ptr;
    g_handle_table[slot].generation++;
    g_handle_table[slot].region_id = region->region_id;
    g_handle_table[slot].active = 1;

    // Create handle
    handle.region_id = region->region_id;
    handle.generation = g_handle_table[slot].generation;
    handle.slot_index = slot;

    g_next_handle_slot = (slot + 1) % SEEN_MAX_HANDLES;

    return handle;
}

void* __seen_region_deref_handle(SeenRegionHandle handle) {
    if (handle.slot_index < 0 || handle.slot_index >= SEEN_MAX_HANDLES) {
        return NULL;
    }

    HandleSlot* slot = &g_handle_table[handle.slot_index];

    // Check if handle is valid
    if (!slot->active) {
        return NULL;
    }

    if (slot->generation != handle.generation) {
        // Handle is stale (generation mismatch)
        return NULL;
    }

    if (slot->region_id != handle.region_id) {
        // Region mismatch
        return NULL;
    }

    return slot->ptr;
}

int __seen_region_handle_valid(SeenRegionHandle handle) {
    if (handle.slot_index < 0 || handle.slot_index >= SEEN_MAX_HANDLES) {
        return 0;
    }

    HandleSlot* slot = &g_handle_table[handle.slot_index];

    if (!slot->active) {
        return 0;
    }

    if (slot->generation != handle.generation) {
        return 0;
    }

    if (slot->region_id != handle.region_id) {
        return 0;
    }

    return 1;
}

// ============================================================================
// Debug/Profiling
// ============================================================================

void __seen_region_print_stats(SeenRegion* region) {
    if (!region) {
        fprintf(stderr, "Region: (null)\n");
        return;
    }

    fprintf(stderr, "Region #%ld%s%s%s:\n",
            (long)region->region_id,
            region->name ? " (" : "",
            region->name ? region->name : "",
            region->name ? ")" : "");
    fprintf(stderr, "  Active: %s\n", region->active ? "yes" : "no");
    fprintf(stderr, "  Strategy: %d\n", region->strategy);
    fprintf(stderr, "  Size: %zu bytes (%.2f MB)\n",
            region->size, (double)region->size / (1024 * 1024));
    fprintf(stderr, "  Used: %zu bytes (%.1f%%)\n",
            region->offset,
            region->size > 0 ? (100.0 * region->offset / region->size) : 0);
    fprintf(stderr, "  Peak: %zu bytes (%.1f%%)\n",
            region->peak_usage,
            region->size > 0 ? (100.0 * region->peak_usage / region->size) : 0);
    fprintf(stderr, "  Allocations: %zu\n", region->alloc_count);
    fprintf(stderr, "  Budget: %s\n",
            region->budget > 0 ? "set" : "none");
    if (region->budget > 0) {
        fprintf(stderr, "  Budget limit: %zu bytes (%.2f MB)\n",
                region->budget, (double)region->budget / (1024 * 1024));
    }
    fprintf(stderr, "  Remaining: %zu bytes\n", region->size - region->offset);
}

void __seen_region_print_all(void) {
    fprintf(stderr, "\n=== SEEN REGION STATUS ===\n");
    fprintf(stderr, "Active regions: %d\n", g_region_count);
    fprintf(stderr, "Stack depth: %d\n", g_region_stack_depth);
    fprintf(stderr, "\n");

    for (int i = 0; i < g_region_count; i++) {
        __seen_region_print_stats(g_regions[i]);
        fprintf(stderr, "\n");
    }

    // Count active handles
    int active_handles = 0;
    for (int i = 0; i < SEEN_MAX_HANDLES; i++) {
        if (g_handle_table[i].active) {
            active_handles++;
        }
    }
    fprintf(stderr, "Active handles: %d / %d\n", active_handles, SEEN_MAX_HANDLES);
    fprintf(stderr, "===========================\n");
}

// Print all region stats (public API for seen_perf_report)
void seen_perf_report(void) {
    __seen_region_print_all();
}

// ============================================================================
// Seen-Friendly Wrappers (all int64_t for Seen type system compatibility)
// These bridge from Seen's Int (i64) to the internal ptr-based API
// ============================================================================

int64_t seen_region_create(int64_t size, int64_t strategy) {
    return (int64_t)(uintptr_t)__seen_region_create(size, (int32_t)strategy);
}

int64_t seen_region_alloc(int64_t region_handle, int64_t size, int64_t alignment) {
    return (int64_t)(uintptr_t)__seen_region_alloc(
        (SeenRegion*)(uintptr_t)region_handle, size, alignment);
}

void seen_region_destroy(int64_t region_handle) {
    __seen_region_destroy((SeenRegion*)(uintptr_t)region_handle);
}

void seen_region_reset(int64_t region_handle) {
    __seen_region_reset((SeenRegion*)(uintptr_t)region_handle);
}

int64_t seen_region_used(int64_t region_handle) {
    return __seen_region_used((SeenRegion*)(uintptr_t)region_handle);
}

int64_t seen_region_remaining(int64_t region_handle) {
    return __seen_region_remaining((SeenRegion*)(uintptr_t)region_handle);
}

int64_t seen_region_peak(int64_t region_handle) {
    return __seen_region_peak((SeenRegion*)(uintptr_t)region_handle);
}

void seen_region_push(int64_t region_handle) {
    __seen_region_push((SeenRegion*)(uintptr_t)region_handle);
}

int64_t seen_region_pop(void) {
    return (int64_t)(uintptr_t)__seen_region_pop();
}

int64_t seen_region_current(void) {
    return (int64_t)(uintptr_t)__seen_region_current();
}

int64_t seen_region_alloc_handle(int64_t region_handle, int64_t size) {
    SeenRegion* region = (SeenRegion*)(uintptr_t)region_handle;
    SeenRegionHandle h = __seen_region_alloc_handle(region, size);
    // Pack 3 fields into single int64:
    //   bits  0-15: slot_index (max 65535)
    //   bits 16-31: generation (mod 65536)
    //   bits 32-63: region_id
    return ((h.region_id & 0xFFFFFFFF) << 32)
         | ((h.generation & 0xFFFF) << 16)
         | (h.slot_index & 0xFFFF);
}

static SeenRegionHandle unpack_handle(int64_t packed) {
    SeenRegionHandle h;
    h.slot_index = packed & 0xFFFF;
    h.generation = (packed >> 16) & 0xFFFF;
    h.region_id = (packed >> 32) & 0xFFFFFFFF;
    return h;
}

int64_t seen_region_deref_handle(int64_t packed_handle) {
    SeenRegionHandle h = unpack_handle(packed_handle);
    void* ptr = __seen_region_deref_handle(h);
    return (int64_t)(uintptr_t)ptr;
}

int64_t seen_region_handle_valid(int64_t packed_handle) {
    SeenRegionHandle h = unpack_handle(packed_handle);
    return __seen_region_handle_valid(h);
}

// ============================================================================
// GPU Region Operations
// ============================================================================

void __seen_region_gpu_sync(SeenRegion* region) {
    if (!region || !region->active) return;
    if (region->strategy != SEEN_REGION_GPU) return;
    // Stub: in a real implementation, this would call vkFlushMappedMemoryRanges
    // or equivalent GPU memory synchronization
    __sync_synchronize();
}

void __seen_region_gpu_map(SeenRegion* region) {
    if (!region || !region->active) return;
    if (region->strategy != SEEN_REGION_GPU) return;
    // Stub: GPU memory is already mapped via mmap; future Vulkan integration
    // would call vkMapMemory here
}

void __seen_region_gpu_unmap(SeenRegion* region) {
    if (!region || !region->active) return;
    if (region->strategy != SEEN_REGION_GPU) return;
    // Stub: future Vulkan integration would call vkUnmapMemory here
}
