// Seen Region Runtime - Memory Region Management
// Provides region-based memory allocation for @preallocate decorator
// This implements Vale-style region memory management

#ifndef SEEN_REGION_H
#define SEEN_REGION_H

#include <stdint.h>
#include <stddef.h>

// ============================================================================
// Region Strategy Enum
// ============================================================================

// Region allocation strategy (matches Seen RegionStrategy enum)
typedef enum {
    SEEN_REGION_AUTO = 0,      // Automatic strategy selection
    SEEN_REGION_BUMP = 1,      // Bump allocator (fast, no individual free)
    SEEN_REGION_STACK = 2,     // Stack-like allocation (LIFO free)
    SEEN_REGION_CXL_NEAR = 3,  // CXL memory - near tier
    SEEN_REGION_CXL_FAR = 4,   // CXL memory - far tier
    SEEN_REGION_GPU = 5         // GPU-visible memory (page-aligned for Vulkan mapping)
} SeenRegionStrategy;

// ============================================================================
// Forward Declarations for Pinning
// ============================================================================

// Forward declare pinning attributes (defined in seen_pinning.h)
struct SeenPinningAttrs;

// ============================================================================
// Region Structure
// ============================================================================

// A memory region with bump allocation
typedef struct SeenRegion {
    void* base;                     // Base address of allocated memory
    size_t size;                    // Total size of region
    size_t offset;                  // Current allocation offset (bump pointer)
    size_t peak_usage;              // Peak usage for profiling
    size_t alloc_count;             // Number of allocations
    size_t budget;                  // Memory budget (0 = no budget)
    int64_t region_id;              // Unique region ID
    int64_t parent_id;              // Parent region ID (-1 if none)
    SeenRegionStrategy strategy;    // Allocation strategy
    int active;                     // Whether region is active
    const char* name;               // Debug name for the region
    struct SeenPinningAttrs* pinning_attrs;  // VSD pinning attributes (optional)
} SeenRegion;

// ============================================================================
// Generational Handle (for safe references)
// ============================================================================

// Handle to memory in a region with generation tracking
// Provides safety against use-after-free
typedef struct SeenRegionHandle {
    int64_t region_id;      // Region this handle belongs to
    int64_t generation;     // Generation counter (incremented on free)
    int64_t slot_index;     // Slot index in handle table
} SeenRegionHandle;

// ============================================================================
// Region Management Functions
// ============================================================================

// Create a new region with specified size and strategy
// Returns pointer to region, or NULL on failure
SeenRegion* __seen_region_create(int64_t size, int32_t strategy);

// Create a named region (for debugging)
SeenRegion* __seen_region_create_named(int64_t size, int32_t strategy, const char* name);

// Allocate memory from a region
// Returns pointer to allocated memory, or NULL if region is full
void* __seen_region_alloc(SeenRegion* region, int64_t size, int64_t alignment);

// Destroy a region and free all memory
// Invalidates all handles pointing to this region
void __seen_region_destroy(SeenRegion* region);

// Reset a region (keeps memory, resets offset to 0)
// Useful for object pools
void __seen_region_reset(SeenRegion* region);

// Get remaining capacity in bytes
int64_t __seen_region_remaining(SeenRegion* region);

// Get total allocated bytes
int64_t __seen_region_used(SeenRegion* region);

// Get peak usage
int64_t __seen_region_peak(SeenRegion* region);

// Get allocation count
int64_t __seen_region_alloc_count(SeenRegion* region);

// Set memory budget (0 = no budget)
void __seen_region_set_budget(SeenRegion* region, int64_t budget_bytes);

// ============================================================================
// Region Stack (for nested regions like function-scoped allocations)
// ============================================================================

// Push a region onto the implicit region stack
void __seen_region_push(SeenRegion* region);

// Pop and return the top region from the stack
SeenRegion* __seen_region_pop(void);

// Get the current (top) region without popping
SeenRegion* __seen_region_current(void);

// Get the global default region (created at program startup)
SeenRegion* __seen_region_global(void);

// GPU region operations
void __seen_region_gpu_sync(SeenRegion* region);
void __seen_region_gpu_map(SeenRegion* region);
void __seen_region_gpu_unmap(SeenRegion* region);

// ============================================================================
// Generational Handle Functions (for safe region references)
// ============================================================================

// Allocate memory and return a generational handle
SeenRegionHandle __seen_region_alloc_handle(SeenRegion* region, int64_t size);

// Dereference a handle to get the memory pointer
// Returns NULL if handle is invalid (stale generation)
void* __seen_region_deref_handle(SeenRegionHandle handle);

// Check if a handle is still valid
int __seen_region_handle_valid(SeenRegionHandle handle);

// ============================================================================
// Debug/Profiling Functions
// ============================================================================

// Print region statistics to stderr
void __seen_region_print_stats(SeenRegion* region);

// Print all active regions
void __seen_region_print_all(void);

// Initialize region subsystem (called automatically)
void __seen_region_init(void);

#endif // SEEN_REGION_H
