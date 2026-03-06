// Seen Pinning Runtime Implementation
// VSD Pointer Pinning for zero-copy shard access
// Part of UWW Infrastructure (Task 5.5)

#ifdef __linux__
#define _GNU_SOURCE  // For MAP_HUGETLB, madvise flags
#endif

#include "seen_pinning.h"
#include "seen_region.h"
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

// Platform-specific includes
#ifdef __linux__
#include <sys/mman.h>
#include <sys/resource.h>
#include <unistd.h>
#include <errno.h>

// NUMA support (optional)
#ifdef HAVE_NUMA
#include <numa.h>
#include <numaif.h>
#endif

#elif defined(__APPLE__)
#include <sys/mman.h>
#include <sys/resource.h>
#include <unistd.h>
#include <errno.h>

#endif // platform includes

// ============================================================================
// Global State
// ============================================================================

// Track total pinned memory
static size_t g_total_pinned_memory = 0;

// ============================================================================
// Initialization
// ============================================================================

void __seen_pinning_init_attrs(SeenPinningAttrs* attrs) {
    if (!attrs) return;

    attrs->pinned = 0;
    attrs->numa_node = SEEN_NUMA_AUTO;
    attrs->use_huge_pages = 0;
    attrs->read_only_after_init = 0;
    attrs->no_swap = 0;
    attrs->prefault = 0;
    attrs->pin_handle = NULL;
    attrs->pinned_size = 0;
}

// ============================================================================
// Memory Pinning Implementation
// ============================================================================

SeenPinStatus __seen_region_pin_memory(SeenRegion* region) {
    if (!region || !region->base || !region->active) {
        return SEEN_PIN_ERR_NULL_REGION;
    }

#ifdef __linux__
    // Check current pinning status
    if (region->pinning_attrs && region->pinning_attrs->pinned) {
        return SEEN_PIN_SUCCESS;  // Already pinned
    }

    // Allocate pinning attributes if not present
    if (!region->pinning_attrs) {
        region->pinning_attrs = (SeenPinningAttrs*)malloc(sizeof(SeenPinningAttrs));
        if (!region->pinning_attrs) {
            return SEEN_PIN_ERR_NO_MEMORY;
        }
        __seen_pinning_init_attrs(region->pinning_attrs);
    }

    // Check memory lock limit
    struct rlimit rlim;
    if (getrlimit(RLIMIT_MEMLOCK, &rlim) == 0) {
        size_t new_total = g_total_pinned_memory + region->size;
        if (rlim.rlim_cur != RLIM_INFINITY && new_total > rlim.rlim_cur) {
            fprintf(stderr, "WARNING: Pinning would exceed RLIMIT_MEMLOCK "
                    "(current: %zu, limit: %lu)\n",
                    new_total, (unsigned long)rlim.rlim_cur);
            return SEEN_PIN_ERR_LIMIT;
        }
    }

    // Lock memory pages
    if (mlock(region->base, region->size) != 0) {
        int err = errno;
        if (err == ENOMEM) {
            return SEEN_PIN_ERR_NO_MEMORY;
        } else if (err == EPERM) {
            return SEEN_PIN_ERR_PERMISSION;
        }
        return SEEN_PIN_ERR_INVALID;
    }

    // Update tracking
    region->pinning_attrs->pinned = 1;
    region->pinning_attrs->pinned_size = region->size;
    g_total_pinned_memory += region->size;

    return SEEN_PIN_SUCCESS;

#elif defined(__APPLE__)
    // Check current pinning status
    if (region->pinning_attrs && region->pinning_attrs->pinned) {
        return SEEN_PIN_SUCCESS;  // Already pinned
    }

    // Allocate pinning attributes if not present
    if (!region->pinning_attrs) {
        region->pinning_attrs = (SeenPinningAttrs*)malloc(sizeof(SeenPinningAttrs));
        if (!region->pinning_attrs) {
            return SEEN_PIN_ERR_NO_MEMORY;
        }
        __seen_pinning_init_attrs(region->pinning_attrs);
    }

    // Check memory lock limit
    struct rlimit rlim;
    if (getrlimit(RLIMIT_MEMLOCK, &rlim) == 0) {
        size_t new_total = g_total_pinned_memory + region->size;
        if (rlim.rlim_cur != RLIM_INFINITY && new_total > rlim.rlim_cur) {
            fprintf(stderr, "WARNING: Pinning would exceed RLIMIT_MEMLOCK "
                    "(current: %zu, limit: %lu)\n",
                    new_total, (unsigned long)rlim.rlim_cur);
            return SEEN_PIN_ERR_LIMIT;
        }
    }

    // Lock memory pages (mlock is available on macOS)
    if (mlock(region->base, region->size) != 0) {
        int err = errno;
        if (err == ENOMEM) {
            return SEEN_PIN_ERR_NO_MEMORY;
        } else if (err == EPERM) {
            return SEEN_PIN_ERR_PERMISSION;
        }
        return SEEN_PIN_ERR_INVALID;
    }

    // Update tracking
    region->pinning_attrs->pinned = 1;
    region->pinning_attrs->pinned_size = region->size;
    g_total_pinned_memory += region->size;

    return SEEN_PIN_SUCCESS;

#else
    // Platform not supported
    return SEEN_PIN_ERR_UNSUPPORTED;
#endif
}

SeenPinStatus __seen_region_unpin_memory(SeenRegion* region) {
    if (!region || !region->base || !region->active) {
        return SEEN_PIN_ERR_NULL_REGION;
    }

#ifdef __linux__
    if (!region->pinning_attrs || !region->pinning_attrs->pinned) {
        return SEEN_PIN_SUCCESS;  // Not pinned
    }

    // Unlock memory pages
    if (munlock(region->base, region->size) != 0) {
        return SEEN_PIN_ERR_INVALID;
    }

    // Update tracking
    if (g_total_pinned_memory >= region->pinning_attrs->pinned_size) {
        g_total_pinned_memory -= region->pinning_attrs->pinned_size;
    }
    region->pinning_attrs->pinned = 0;
    region->pinning_attrs->pinned_size = 0;

    return SEEN_PIN_SUCCESS;

#elif defined(__APPLE__)
    if (!region->pinning_attrs || !region->pinning_attrs->pinned) {
        return SEEN_PIN_SUCCESS;  // Not pinned
    }

    // Unlock memory pages (munlock is available on macOS)
    if (munlock(region->base, region->size) != 0) {
        return SEEN_PIN_ERR_INVALID;
    }

    // Update tracking
    if (g_total_pinned_memory >= region->pinning_attrs->pinned_size) {
        g_total_pinned_memory -= region->pinning_attrs->pinned_size;
    }
    region->pinning_attrs->pinned = 0;
    region->pinning_attrs->pinned_size = 0;

    return SEEN_PIN_SUCCESS;

#else
    return SEEN_PIN_ERR_UNSUPPORTED;
#endif
}

int __seen_region_is_pinned(SeenRegion* region) {
    if (!region || !region->pinning_attrs) {
        return 0;
    }
    return region->pinning_attrs->pinned;
}

// ============================================================================
// NUMA Implementation
// ============================================================================

SeenPinStatus __seen_region_set_numa_affinity(SeenRegion* region, int node) {
    if (!region || !region->base || !region->active) {
        return SEEN_PIN_ERR_NULL_REGION;
    }

#if defined(__linux__) && defined(HAVE_NUMA)
    if (!numa_available()) {
        return SEEN_PIN_ERR_NUMA;
    }

    // Allocate pinning attributes if not present
    if (!region->pinning_attrs) {
        region->pinning_attrs = (SeenPinningAttrs*)malloc(sizeof(SeenPinningAttrs));
        if (!region->pinning_attrs) {
            return SEEN_PIN_ERR_NO_MEMORY;
        }
        __seen_pinning_init_attrs(region->pinning_attrs);
    }

    if (node == SEEN_NUMA_AUTO) {
        // Use interleaved policy
        unsigned long all_nodes = (1UL << numa_max_node()) - 1;
        if (mbind(region->base, region->size, MPOL_INTERLEAVE,
                  &all_nodes, sizeof(all_nodes) * 8,
                  MPOL_MF_MOVE) != 0) {
            return SEEN_PIN_ERR_NUMA;
        }
    } else {
        // Bind to specific node
        unsigned long nodemask = 1UL << node;
        if (mbind(region->base, region->size, MPOL_BIND,
                  &nodemask, sizeof(nodemask) * 8,
                  MPOL_MF_MOVE) != 0) {
            return SEEN_PIN_ERR_NUMA;
        }
    }

    region->pinning_attrs->numa_node = node;
    return SEEN_PIN_SUCCESS;

#else
    (void)node;  // Suppress unused warning
    // NUMA not available - just record the preference
    if (region->pinning_attrs) {
        region->pinning_attrs->numa_node = node;
    }
    return SEEN_PIN_SUCCESS;  // Silently succeed without NUMA
#endif
}

int __seen_region_get_numa_node(SeenRegion* region) {
    if (!region || !region->pinning_attrs) {
        return SEEN_NUMA_AUTO;
    }
    return region->pinning_attrs->numa_node;
}

int __seen_get_numa_node_count(void) {
#if defined(__linux__) && defined(HAVE_NUMA)
    if (!numa_available()) {
        return 1;
    }
    return numa_max_node() + 1;
#else
    return 1;  // Single node
#endif
}

int __seen_numa_available(void) {
#if defined(__linux__) && defined(HAVE_NUMA)
    return numa_available() >= 0;
#else
    return 0;
#endif
}

// ============================================================================
// Huge Pages Implementation
// ============================================================================

SeenPinStatus __seen_region_use_huge_pages(SeenRegion* region) {
    if (!region || !region->base || !region->active) {
        return SEEN_PIN_ERR_NULL_REGION;
    }

#ifdef __linux__
    // Allocate pinning attributes if not present
    if (!region->pinning_attrs) {
        region->pinning_attrs = (SeenPinningAttrs*)malloc(sizeof(SeenPinningAttrs));
        if (!region->pinning_attrs) {
            return SEEN_PIN_ERR_NO_MEMORY;
        }
        __seen_pinning_init_attrs(region->pinning_attrs);
    }

    // Use madvise to enable transparent huge pages
#ifdef MADV_HUGEPAGE
    if (madvise(region->base, region->size, MADV_HUGEPAGE) != 0) {
        return SEEN_PIN_ERR_HUGE_PAGES;
    }
    region->pinning_attrs->use_huge_pages = 1;
    return SEEN_PIN_SUCCESS;
#else
    return SEEN_PIN_ERR_UNSUPPORTED;
#endif

#elif defined(__APPLE__)
    // macOS handles superpage promotion transparently for large aligned allocations.
    // Mark region for sequential access to hint the kernel.
    if (!region->pinning_attrs) {
        region->pinning_attrs = (SeenPinningAttrs*)malloc(sizeof(SeenPinningAttrs));
        if (!region->pinning_attrs) {
            return SEEN_PIN_ERR_NO_MEMORY;
        }
        __seen_pinning_init_attrs(region->pinning_attrs);
    }
    madvise(region->base, region->size, MADV_SEQUENTIAL);
    region->pinning_attrs->use_huge_pages = 1;
    return SEEN_PIN_SUCCESS;
#else
    return SEEN_PIN_ERR_UNSUPPORTED;
#endif
}

SeenPinStatus __seen_region_disable_huge_pages(SeenRegion* region) {
    if (!region || !region->base || !region->active) {
        return SEEN_PIN_ERR_NULL_REGION;
    }

#ifdef __linux__
#ifdef MADV_NOHUGEPAGE
    if (madvise(region->base, region->size, MADV_NOHUGEPAGE) != 0) {
        return SEEN_PIN_ERR_HUGE_PAGES;
    }
    if (region->pinning_attrs) {
        region->pinning_attrs->use_huge_pages = 0;
    }
    return SEEN_PIN_SUCCESS;
#else
    return SEEN_PIN_ERR_UNSUPPORTED;
#endif

#else
    return SEEN_PIN_ERR_UNSUPPORTED;
#endif
}

int __seen_region_has_huge_pages(SeenRegion* region) {
    if (!region || !region->pinning_attrs) {
        return 0;
    }
    return region->pinning_attrs->use_huge_pages;
}

size_t __seen_get_huge_page_size(void) {
#ifdef __linux__
    // Read from /proc/meminfo
    FILE* f = fopen("/proc/meminfo", "r");
    if (f) {
        char line[256];
        while (fgets(line, sizeof(line), f)) {
            size_t size_kb;
            if (sscanf(line, "Hugepagesize: %zu kB", &size_kb) == 1) {
                fclose(f);
                return size_kb * 1024;
            }
        }
        fclose(f);
    }
    return SEEN_HUGE_PAGE_SIZE;  // Default 2MB
#else
    return SEEN_HUGE_PAGE_SIZE;
#endif
}

int __seen_huge_pages_available(void) {
#ifdef __linux__
    SeenHugePageStats stats;
    __seen_get_huge_page_stats(&stats);
    return stats.free_pages > 0;
#elif defined(__APPLE__)
    // macOS kernel supports superpages transparently on Apple Silicon
    return 1;
#else
    return 0;
#endif
}

// ============================================================================
// Memory Protection Implementation
// ============================================================================

SeenPinStatus __seen_region_make_read_only(SeenRegion* region) {
    if (!region || !region->base || !region->active) {
        return SEEN_PIN_ERR_NULL_REGION;
    }

#if defined(__linux__) || defined(__APPLE__)
    if (mprotect(region->base, region->size, PROT_READ) != 0) {
        return SEEN_PIN_ERR_PERMISSION;
    }
    if (region->pinning_attrs) {
        region->pinning_attrs->read_only_after_init = 1;
    }
    return SEEN_PIN_SUCCESS;
#else
    return SEEN_PIN_ERR_UNSUPPORTED;
#endif
}

SeenPinStatus __seen_region_make_read_write(SeenRegion* region) {
    if (!region || !region->base || !region->active) {
        return SEEN_PIN_ERR_NULL_REGION;
    }

#if defined(__linux__) || defined(__APPLE__)
    if (mprotect(region->base, region->size, PROT_READ | PROT_WRITE) != 0) {
        return SEEN_PIN_ERR_PERMISSION;
    }
    if (region->pinning_attrs) {
        region->pinning_attrs->read_only_after_init = 0;
    }
    return SEEN_PIN_SUCCESS;
#else
    return SEEN_PIN_ERR_UNSUPPORTED;
#endif
}

SeenPinStatus __seen_region_prefault(SeenRegion* region) {
    if (!region || !region->base || !region->active) {
        return SEEN_PIN_ERR_NULL_REGION;
    }

#if defined(__linux__) || defined(__APPLE__)
    // Touch every page to ensure it's allocated
    volatile char* p = (volatile char*)region->base;
    size_t page_size = sysconf(_SC_PAGESIZE);
    for (size_t i = 0; i < region->size; i += page_size) {
        (void)p[i];  // Read to trigger page fault
    }
    if (region->pinning_attrs) {
        region->pinning_attrs->prefault = 1;
    }
    return SEEN_PIN_SUCCESS;
#else
    return SEEN_PIN_ERR_UNSUPPORTED;
#endif
}

SeenPinStatus __seen_region_advise_sequential(SeenRegion* region) {
    if (!region || !region->base || !region->active) {
        return SEEN_PIN_ERR_NULL_REGION;
    }

#if defined(__linux__) || defined(__APPLE__)
    if (madvise(region->base, region->size, MADV_SEQUENTIAL) != 0) {
        return SEEN_PIN_ERR_INVALID;
    }
    return SEEN_PIN_SUCCESS;
#else
    return SEEN_PIN_ERR_UNSUPPORTED;
#endif
}

SeenPinStatus __seen_region_advise_random(SeenRegion* region) {
    if (!region || !region->base || !region->active) {
        return SEEN_PIN_ERR_NULL_REGION;
    }

#if defined(__linux__) || defined(__APPLE__)
    if (madvise(region->base, region->size, MADV_RANDOM) != 0) {
        return SEEN_PIN_ERR_INVALID;
    }
    return SEEN_PIN_SUCCESS;
#else
    return SEEN_PIN_ERR_UNSUPPORTED;
#endif
}

SeenPinStatus __seen_region_advise_willneed(SeenRegion* region) {
    if (!region || !region->base || !region->active) {
        return SEEN_PIN_ERR_NULL_REGION;
    }

#if defined(__linux__) || defined(__APPLE__)
    if (madvise(region->base, region->size, MADV_WILLNEED) != 0) {
        return SEEN_PIN_ERR_INVALID;
    }
    return SEEN_PIN_SUCCESS;
#else
    return SEEN_PIN_ERR_UNSUPPORTED;
#endif
}

SeenPinStatus __seen_region_advise_dontneed(SeenRegion* region) {
    if (!region || !region->base || !region->active) {
        return SEEN_PIN_ERR_NULL_REGION;
    }

#if defined(__linux__) || defined(__APPLE__)
    if (madvise(region->base, region->size, MADV_DONTNEED) != 0) {
        return SEEN_PIN_ERR_INVALID;
    }
    return SEEN_PIN_SUCCESS;
#else
    return SEEN_PIN_ERR_UNSUPPORTED;
#endif
}

// ============================================================================
// Status and Diagnostics
// ============================================================================

size_t __seen_get_total_pinned_memory(void) {
    return g_total_pinned_memory;
}

size_t __seen_get_memlock_limit(void) {
#if defined(__linux__) || defined(__APPLE__)
    struct rlimit rlim;
    if (getrlimit(RLIMIT_MEMLOCK, &rlim) == 0) {
        return rlim.rlim_cur;
    }
#endif
    return 0;
}

void __seen_get_huge_page_stats(SeenHugePageStats* stats) {
    if (!stats) return;

    memset(stats, 0, sizeof(*stats));

#ifdef __linux__
    FILE* f = fopen("/proc/meminfo", "r");
    if (f) {
        char line[256];
        while (fgets(line, sizeof(line), f)) {
            size_t val;
            if (sscanf(line, "HugePages_Total: %zu", &val) == 1) {
                stats->total_pages = val;
            } else if (sscanf(line, "HugePages_Free: %zu", &val) == 1) {
                stats->free_pages = val;
            } else if (sscanf(line, "HugePages_Rsvd: %zu", &val) == 1) {
                stats->reserved_pages = val;
            } else if (sscanf(line, "HugePages_Surp: %zu", &val) == 1) {
                stats->surplus_pages = val;
            }
        }
        fclose(f);
    }
#endif
}

const char* __seen_pin_status_to_string(SeenPinStatus status) {
    switch (status) {
        case SEEN_PIN_SUCCESS:        return "Success";
        case SEEN_PIN_ERR_NULL_REGION: return "Null region";
        case SEEN_PIN_ERR_NO_MEMORY:  return "Insufficient memory";
        case SEEN_PIN_ERR_PERMISSION: return "Permission denied (need CAP_IPC_LOCK)";
        case SEEN_PIN_ERR_LIMIT:      return "RLIMIT_MEMLOCK exceeded";
        case SEEN_PIN_ERR_INVALID:    return "Invalid parameters";
        case SEEN_PIN_ERR_UNSUPPORTED: return "Not supported on this platform";
        case SEEN_PIN_ERR_NUMA:       return "NUMA operation failed";
        case SEEN_PIN_ERR_HUGE_PAGES: return "Huge pages not available";
        default:                      return "Unknown error";
    }
}

void __seen_region_print_pinning_info(SeenRegion* region) {
    if (!region) {
        fprintf(stderr, "Region: (null)\n");
        return;
    }

    fprintf(stderr, "Region #%ld Pinning Info:\n", (long)region->region_id);

    if (!region->pinning_attrs) {
        fprintf(stderr, "  No pinning attributes\n");
        return;
    }

    SeenPinningAttrs* attrs = region->pinning_attrs;
    fprintf(stderr, "  Pinned: %s\n", attrs->pinned ? "yes" : "no");
    if (attrs->pinned) {
        fprintf(stderr, "  Pinned Size: %zu bytes\n", attrs->pinned_size);
    }
    fprintf(stderr, "  NUMA Node: %d%s\n",
            attrs->numa_node,
            attrs->numa_node == SEEN_NUMA_AUTO ? " (auto)" : "");
    fprintf(stderr, "  Huge Pages: %s\n", attrs->use_huge_pages ? "yes" : "no");
    fprintf(stderr, "  Read-Only: %s\n", attrs->read_only_after_init ? "yes" : "no");
    fprintf(stderr, "  Prefaulted: %s\n", attrs->prefault ? "yes" : "no");
}

// ============================================================================
// Extended Region Creation
// ============================================================================

SeenRegion* __seen_region_create_pinned(
    int64_t size,
    int32_t strategy,
    const char* name,
    const SeenPinningAttrs* attrs
) {
    // Create base region
    SeenRegion* region = __seen_region_create_named(size, strategy, name);
    if (!region) {
        return NULL;
    }

    // Copy pinning attributes
    if (attrs) {
        region->pinning_attrs = (SeenPinningAttrs*)malloc(sizeof(SeenPinningAttrs));
        if (!region->pinning_attrs) {
            __seen_region_destroy(region);
            return NULL;
        }
        memcpy(region->pinning_attrs, attrs, sizeof(SeenPinningAttrs));
        region->pinning_attrs->pin_handle = NULL;
        region->pinning_attrs->pinned = 0;
        region->pinning_attrs->pinned_size = 0;

        // Apply pinning if requested
        if (attrs->pinned) {
            SeenPinStatus status = __seen_region_pin_memory(region);
            if (status != SEEN_PIN_SUCCESS) {
                fprintf(stderr, "WARNING: Failed to pin memory: %s\n",
                        __seen_pin_status_to_string(status));
            }
        }

        // Apply NUMA affinity if specified
        if (attrs->numa_node != SEEN_NUMA_AUTO) {
            SeenPinStatus status = __seen_region_set_numa_affinity(region, attrs->numa_node);
            if (status != SEEN_PIN_SUCCESS) {
                fprintf(stderr, "WARNING: Failed to set NUMA affinity: %s\n",
                        __seen_pin_status_to_string(status));
            }
        }

        // Apply huge pages if requested
        if (attrs->use_huge_pages) {
            SeenPinStatus status = __seen_region_use_huge_pages(region);
            if (status != SEEN_PIN_SUCCESS) {
                fprintf(stderr, "WARNING: Failed to enable huge pages: %s\n",
                        __seen_pin_status_to_string(status));
            }
        }

        // Prefault if requested
        if (attrs->prefault) {
            __seen_region_prefault(region);
        }
    }

    return region;
}
