// Seen NUMA Runtime - NUMA-Aware Memory Allocation
// Provides NUMA (Non-Uniform Memory Access) support for the Seen runtime
//
// This module detects NUMA topology at startup and provides:
// - Node-local memory allocation
// - Interleaved allocation across nodes
// - Thread-to-NUMA node affinity bindings
//
// On Linux with libnuma, full NUMA support is available.
// On other platforms, graceful fallbacks to standard allocation are provided.

#ifndef SEEN_NUMA_H
#define SEEN_NUMA_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

// ============================================================================
// NUMA Topology Information
// ============================================================================

// Maximum number of NUMA nodes supported
#define SEEN_NUMA_MAX_NODES 128

// Maximum number of CPUs supported
#define SEEN_NUMA_MAX_CPUS 1024

// NUMA node information
typedef struct {
    int node_id;                    // NUMA node ID (-1 if unavailable)
    size_t total_memory;            // Total memory on node (0 if unknown)
    size_t free_memory;             // Free memory on node (0 if unknown)
    int num_cpus;                   // Number of CPUs on this node
    int cpus[SEEN_NUMA_MAX_CPUS];   // CPU IDs assigned to this node
    bool is_available;              // Whether this node is available
} SeenNUMANodeInfo;

// NUMA topology structure
typedef struct {
    int num_nodes;                  // Total number of NUMA nodes (1 = no NUMA)
    int num_cpus;                   // Total number of CPUs
    bool numa_available;            // Whether NUMA is available on this system
    bool initialized;               // Whether topology has been detected
    SeenNUMANodeInfo nodes[SEEN_NUMA_MAX_NODES];
} SeenNUMATopology;

// NUMA allocation strategies
typedef enum {
    SEEN_NUMA_STRATEGY_LOCAL = 0,       // Allocate on current node (default)
    SEEN_NUMA_STRATEGY_INTERLEAVED = 1, // Allocate interleaved across all nodes
    SEEN_NUMA_STRATEGY_BIND = 2,        // Force allocation on specific node (fail if not possible)
    SEEN_NUMA_STRATEGY_PREFERRED = 3,   // Prefer specific node, fallback to others
} SeenNUMAStrategy;

// ============================================================================
// NUMA Initialization and Topology Detection
// ============================================================================

// Initialize NUMA subsystem and detect topology
// Called automatically on first use, but can be called explicitly
void seen_numa_init(void);

// Get the detected NUMA topology
// Returns NULL if not initialized
const SeenNUMATopology* seen_numa_get_topology(void);

// Check if NUMA is available on this system
bool seen_numa_is_available(void);

// Get the number of NUMA nodes
int seen_numa_num_nodes(void);

// Get the current thread's NUMA node (-1 if unknown)
int seen_numa_current_node(void);

// Get NUMA node info for a specific node
// Returns NULL if node_id is invalid
const SeenNUMANodeInfo* seen_numa_get_node_info(int node_id);

// Print NUMA topology information to stderr
void seen_numa_print_topology(void);

// ============================================================================
// NUMA-Aware Memory Allocation
// ============================================================================

// Allocate memory on a specific NUMA node
// If NUMA is not available, falls back to standard malloc
// node: NUMA node ID, or -1 for current node
void* seen_numa_alloc_onnode(size_t size, int node);

// Allocate memory interleaved across all NUMA nodes
// If NUMA is not available, falls back to standard malloc
void* seen_numa_alloc_interleaved(size_t size);

// Allocate memory on the current NUMA node
// If NUMA is not available, falls back to standard malloc
void* seen_numa_alloc_local(size_t size);

// Allocate aligned memory on a specific NUMA node
// alignment must be a power of 2
void* seen_numa_alloc_aligned(size_t size, size_t alignment, int node);

// Free memory allocated by NUMA allocation functions
// Safe to call even if NUMA is not available
void seen_numa_free(void* ptr, size_t size);

// Reallocate NUMA memory
// Note: may move memory to a different node
void* seen_numa_realloc(void* ptr, size_t old_size, size_t new_size, int node);

// Get the NUMA node of a memory address (-1 if unknown)
int seen_numa_get_node(void* ptr);

// Move memory to a different NUMA node (if supported)
// Returns 0 on success, -1 on failure
int seen_numa_move_pages(void* ptr, size_t size, int target_node);

// ============================================================================
// Thread Affinity Bindings
// ============================================================================

// Bind current thread to a specific NUMA node
// Returns true on success, false on failure
bool seen_numa_bind_thread_to_node(int node);

// Bind current thread to specific CPUs
// cpus: array of CPU IDs
// num_cpus: number of CPUs in the array
// Returns true on success, false on failure
bool seen_numa_bind_thread_to_cpus(const int* cpus, int num_cpus);

// Get current thread's CPU affinity
// cpus: output array to store CPU IDs
// max_cpus: size of the output array
// Returns number of CPUs in the affinity mask, or -1 on error
int seen_numa_get_thread_affinity(int* cpus, int max_cpus);

// Run a function on a specific NUMA node (temporary binding)
// The function is executed with the calling thread bound to the specified node
// After execution, the original binding is restored
void seen_numa_run_on_node(int node, void (*func)(void*), void* arg);

// ============================================================================
// High-Level Memory Policy
// ============================================================================

// Set the default NUMA allocation strategy for the current thread
void seen_numa_set_strategy(SeenNUMAStrategy strategy);

// Get the current NUMA allocation strategy
SeenNUMAStrategy seen_numa_get_strategy(void);

// Allocate memory using the current strategy
void* seen_numa_alloc(size_t size);

// Allocate memory using the current strategy with alignment
void* seen_numa_alloc_aligned_strategy(size_t size, size_t alignment);

// ============================================================================
// Batch/Multi-Page Operations
// ============================================================================

// Allocate multiple pages on specific nodes
// pages: array of page pointers (output)
// num_pages: number of pages to allocate
// page_size: size of each page
// nodes: array of node IDs for each page (can be NULL for current node)
// Returns number of pages successfully allocated
int seen_numa_alloc_pages(void** pages, int num_pages, size_t page_size, const int* nodes);

// Free multiple pages
void seen_numa_free_pages(void** pages, int num_pages, size_t page_size);

// ============================================================================
// Performance Monitoring
// ============================================================================

// NUMA memory statistics for a node
typedef struct {
    int node_id;
    size_t total_bytes;         // Total bytes allocated on this node
    size_t active_bytes;        // Currently active (not freed) bytes
    size_t allocation_count;    // Number of allocations
    size_t free_count;          // Number of frees
} SeenNUMANodeStats;

// Get statistics for a NUMA node
// Returns true on success, false if node_id is invalid
bool seen_numa_get_node_stats(int node_id, SeenNUMANodeStats* stats);

// Reset statistics for all nodes
void seen_numa_reset_stats(void);

// Print NUMA allocation statistics to stderr
void seen_numa_print_stats(void);

// ============================================================================
// Integration with Seen Runtime
// ============================================================================

// Initialize NUMA and set up automatic NUMA-aware allocation
// Called by seen_runtime_init() when NUMA support is enabled
void seen_numa_runtime_init(void);

// Get recommended NUMA node for the current operation
// This uses a round-robin strategy for load balancing
int seen_numa_recommended_node(void);

// Allocate a SeenArray on a specific NUMA node
// Note: This is a convenience wrapper around seen_numa_alloc_onnode
void* seen_numa_array_alloc(int64_t element_size, int64_t capacity, int node);

#ifdef __cplusplus
}
#endif

#endif // SEEN_NUMA_H
