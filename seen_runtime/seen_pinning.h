// Seen Pinning Runtime - VSD Pointer Pinning Implementation
// Provides memory pinning for zero-copy shard access
// Part of UWW Infrastructure (Task 5.5)

#ifndef SEEN_PINNING_H
#define SEEN_PINNING_H

#include <stdint.h>
#include <stddef.h>

// Forward declaration
struct SeenRegion;

// ============================================================================
// Pinning Attributes
// ============================================================================

// Pinning attributes for a memory region
typedef struct SeenPinningAttrs {
    int pinned;                 // Whether memory is mlock'd in RAM
    int numa_node;              // NUMA node affinity (-1 = auto/no preference)
    int use_huge_pages;         // Use transparent huge pages
    int read_only_after_init;   // Mark read-only after initialization
    int no_swap;                // Prevent swapping (via madvise)
    int prefault;               // Prefault pages on allocation
    void* pin_handle;           // OS-specific pin handle
    size_t pinned_size;         // Size of pinned region
} SeenPinningAttrs;

// ============================================================================
// Pinning Status Codes
// ============================================================================

typedef enum SeenPinStatus {
    SEEN_PIN_SUCCESS = 0,           // Operation successful
    SEEN_PIN_ERR_NULL_REGION = -1,  // Null region pointer
    SEEN_PIN_ERR_NO_MEMORY = -2,    // Not enough memory for pinning
    SEEN_PIN_ERR_PERMISSION = -3,   // Insufficient privileges (need CAP_IPC_LOCK)
    SEEN_PIN_ERR_LIMIT = -4,        // RLIMIT_MEMLOCK exceeded
    SEEN_PIN_ERR_INVALID = -5,      // Invalid parameters
    SEEN_PIN_ERR_UNSUPPORTED = -6,  // Operation not supported on this platform
    SEEN_PIN_ERR_NUMA = -7,         // NUMA operation failed
    SEEN_PIN_ERR_HUGE_PAGES = -8    // Huge pages not available
} SeenPinStatus;

// ============================================================================
// Pinning Configuration
// ============================================================================

// Default NUMA node (auto-detect)
#define SEEN_NUMA_AUTO (-1)

// Maximum pinnable memory (256MB default, can be increased)
#define SEEN_MAX_PINNED_MEMORY (256 * 1024 * 1024)

// Huge page size (2MB on x86_64)
#define SEEN_HUGE_PAGE_SIZE (2 * 1024 * 1024)

// ============================================================================
// Memory Pinning Functions
// ============================================================================

// Initialize pinning attributes with defaults
void __seen_pinning_init_attrs(SeenPinningAttrs* attrs);

// Pin region memory in physical RAM (prevents swapping/paging)
// Returns SEEN_PIN_SUCCESS on success, error code on failure
// Requires CAP_IPC_LOCK capability or sufficient RLIMIT_MEMLOCK
SeenPinStatus __seen_region_pin_memory(struct SeenRegion* region);

// Unpin region memory (allow swapping/paging)
SeenPinStatus __seen_region_unpin_memory(struct SeenRegion* region);

// Check if region is currently pinned
int __seen_region_is_pinned(struct SeenRegion* region);

// ============================================================================
// NUMA Functions
// ============================================================================

// Set NUMA node affinity for region memory
// node = -1 for auto/interleave, 0-N for specific node
SeenPinStatus __seen_region_set_numa_affinity(struct SeenRegion* region, int node);

// Get NUMA node where region memory is allocated
int __seen_region_get_numa_node(struct SeenRegion* region);

// Get number of available NUMA nodes
int __seen_get_numa_node_count(void);

// Check if NUMA is available on this system
int __seen_numa_available(void);

// ============================================================================
// Huge Page Functions
// ============================================================================

// Enable transparent huge pages for region
SeenPinStatus __seen_region_use_huge_pages(struct SeenRegion* region);

// Disable transparent huge pages for region
SeenPinStatus __seen_region_disable_huge_pages(struct SeenRegion* region);

// Check if huge pages are being used for region
int __seen_region_has_huge_pages(struct SeenRegion* region);

// Get system huge page size
size_t __seen_get_huge_page_size(void);

// Check if huge pages are available on this system
int __seen_huge_pages_available(void);

// ============================================================================
// Memory Protection Functions
// ============================================================================

// Mark region as read-only (after initialization)
SeenPinStatus __seen_region_make_read_only(struct SeenRegion* region);

// Mark region as read-write
SeenPinStatus __seen_region_make_read_write(struct SeenRegion* region);

// Prefault all pages in region (touch pages to ensure allocation)
SeenPinStatus __seen_region_prefault(struct SeenRegion* region);

// Advise kernel about memory usage patterns
// Sequential: __seen_region_advise_sequential
// Random: __seen_region_advise_random
// WillNeed: __seen_region_advise_willneed
// DontNeed: __seen_region_advise_dontneed
SeenPinStatus __seen_region_advise_sequential(struct SeenRegion* region);
SeenPinStatus __seen_region_advise_random(struct SeenRegion* region);
SeenPinStatus __seen_region_advise_willneed(struct SeenRegion* region);
SeenPinStatus __seen_region_advise_dontneed(struct SeenRegion* region);

// ============================================================================
// Status and Diagnostics
// ============================================================================

// Get total amount of pinned memory across all regions
size_t __seen_get_total_pinned_memory(void);

// Get system memory limit for pinning (RLIMIT_MEMLOCK)
size_t __seen_get_memlock_limit(void);

// Get current system huge pages statistics
typedef struct SeenHugePageStats {
    size_t total_pages;         // Total huge pages configured
    size_t free_pages;          // Free huge pages available
    size_t reserved_pages;      // Reserved huge pages
    size_t surplus_pages;       // Surplus huge pages
} SeenHugePageStats;

void __seen_get_huge_page_stats(SeenHugePageStats* stats);

// Convert status code to string
const char* __seen_pin_status_to_string(SeenPinStatus status);

// Print pinning diagnostics for a region
void __seen_region_print_pinning_info(struct SeenRegion* region);

// ============================================================================
// Extended Region Creation with Pinning
// ============================================================================

// Create a region with pinning attributes
struct SeenRegion* __seen_region_create_pinned(
    int64_t size,
    int32_t strategy,
    const char* name,
    const SeenPinningAttrs* attrs
);

#endif // SEEN_PINNING_H
