// Seen NUMA Runtime Implementation
// NUMA-aware memory allocation for the Seen language runtime

#ifdef __linux__
#define _GNU_SOURCE
#endif
#include "seen_numa.h"
#include "seen_runtime.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <unistd.h>
#include <stdatomic.h>

// Platform detection for NUMA support
#if defined(__linux__) && !defined(SEEN_NUMA_DISABLE_LIBNUMA) && !defined(SEEN_JIT_BUILD)
    #define SEEN_NUMA_USE_LIBNUMA
#endif

// Include libnuma if available
#ifdef SEEN_NUMA_USE_LIBNUMA
    #include <numa.h>
    #include <numaif.h>
    #include <sched.h>
    #include <sys/syscall.h>
    #include <sys/mman.h>
#endif

// ============================================================================
// Global State
// ============================================================================

static SeenNUMATopology g_numa_topology = {0};
#ifdef SEEN_JIT_BUILD
static SeenNUMAStrategy g_numa_strategy = SEEN_NUMA_STRATEGY_LOCAL;
#else
static _Thread_local SeenNUMAStrategy g_numa_strategy = SEEN_NUMA_STRATEGY_LOCAL;
#endif
static volatile int g_numa_initialized = 0;

// Statistics tracking
#ifdef SEEN_JIT_BUILD
static SeenNUMANodeStats g_numa_stats[SEEN_NUMA_MAX_NODES] = {0};
#else
static _Thread_local SeenNUMANodeStats g_numa_stats[SEEN_NUMA_MAX_NODES] = {0};
#endif

// Round-robin counter for load balancing
#ifdef SEEN_JIT_BUILD
static int g_numa_round_robin = 0;
#else
static _Atomic int g_numa_round_robin = 0;
#endif

// ============================================================================
// Fallback Implementations (for non-NUMA systems)
// ============================================================================

#ifndef SEEN_NUMA_USE_LIBNUMA

// Fallback: No NUMA support available
static int fallback_numa_available(void) {
    return 0;
}

static int fallback_numa_num_configured_nodes(void) {
    return 1;
}

static int fallback_numa_num_configured_cpus(void) {
    return (int)sysconf(_SC_NPROCESSORS_ONLN);
}

static int fallback_numa_node_of_cpu(int cpu) {
    (void)cpu;
    return 0;
}

static void* fallback_numa_alloc_onnode(size_t size, int node) {
    (void)node;
    return malloc(size);
}

static void* fallback_numa_alloc_interleaved(size_t size) {
    return malloc(size);
}

static void* fallback_numa_alloc_local(size_t size) {
    return malloc(size);
}

static void fallback_numa_free(void* ptr, size_t size) {
    (void)size;
    free(ptr);
}

static int fallback_numa_run_on_node(int node) {
    (void)node;
    return 0;
}

static int fallback_numa_run_on_node_mask(void* mask) {
    (void)mask;
    return 0;
}

static void* fallback_numa_allocate_nodemask(void) {
    return NULL;
}

static void fallback_numa_bitmask_clearall(void* mask) {
    (void)mask;
}

static void fallback_numa_bitmask_setbit(void* mask, int bit) {
    (void)mask;
    (void)bit;
}

static long fallback_numa_node_size(int node, long* freep) {
    (void)node;
    if (freep) *freep = 0;
    return 0;
}

static int fallback_numa_move_pages(int pid, unsigned long count,
                                     void** pages, const int* nodes,
                                     int* status, int flags) {
    (void)pid;
    (void)count;
    (void)pages;
    (void)nodes;
    (void)status;
    (void)flags;
    return -1;
}

// Map fallback functions to the names used in implementation
// NOTE: These macros are NOT defined for JIT builds to avoid conflicts with struct field names
// NOTE: numa_available is excluded because it conflicts with SeenNUMATopology.numa_available
#ifndef SEEN_JIT_BUILD
#define numa_num_configured_nodes fallback_numa_num_configured_nodes
#define numa_num_configured_cpus fallback_numa_num_configured_cpus
#define numa_node_of_cpu fallback_numa_node_of_cpu
#define numa_alloc_onnode fallback_numa_alloc_onnode
#define numa_alloc_interleaved fallback_numa_alloc_interleaved
#define numa_alloc_local fallback_numa_alloc_local
#define numa_free fallback_numa_free
#define numa_run_on_node fallback_numa_run_on_node
#define numa_run_on_node_mask fallback_numa_run_on_node_mask
#define numa_allocate_nodemask fallback_numa_allocate_nodemask
#define numa_bitmask_clearall fallback_numa_bitmask_clearall
#define numa_bitmask_setbit fallback_numa_bitmask_setbit
#define numa_node_size fallback_numa_node_size
#define numa_move_pages fallback_numa_move_pages
#endif

#endif // !SEEN_NUMA_USE_LIBNUMA

// ============================================================================
// Internal Helper Functions
// ============================================================================

// Get current thread ID
static pid_t seen_gettid(void) {
#ifdef SEEN_NUMA_USE_LIBNUMA
    return syscall(SYS_gettid);
#else
    return getpid();
#endif
}

// Update statistics for an allocation
static void seen_numa_stat_alloc(int node, size_t size) {
    if (node < 0 || node >= SEEN_NUMA_MAX_NODES) return;
    g_numa_stats[node].node_id = node;
    g_numa_stats[node].total_bytes += size;
    g_numa_stats[node].active_bytes += size;
    g_numa_stats[node].allocation_count++;
}

// Update statistics for a free
static void seen_numa_stat_free(int node, size_t size) {
    if (node < 0 || node >= SEEN_NUMA_MAX_NODES) return;
    if (g_numa_stats[node].active_bytes >= size) {
        g_numa_stats[node].active_bytes -= size;
    }
    g_numa_stats[node].free_count++;
}

// ============================================================================
// NUMA Initialization and Topology Detection
// ============================================================================

void seen_numa_init(void) {
    if (g_numa_initialized) return;
    
    memset(&g_numa_topology, 0, sizeof(g_numa_topology));
    
#ifdef SEEN_NUMA_USE_LIBNUMA
    if (numa_available() < 0) {
        // NUMA not available on this system
        g_numa_topology.numa_available = false;
        g_numa_topology.num_nodes = 1;
        g_numa_topology.num_cpus = (int)sysconf(_SC_NPROCESSORS_ONLN);
        g_numa_topology.nodes[0].node_id = 0;
        g_numa_topology.nodes[0].is_available = true;
        g_numa_topology.nodes[0].num_cpus = g_numa_topology.num_cpus;
        
        // Fill in CPU list
        int max_cpus = g_numa_topology.num_cpus < SEEN_NUMA_MAX_CPUS 
                       ? g_numa_topology.num_cpus : SEEN_NUMA_MAX_CPUS;
        for (int i = 0; i < max_cpus; i++) {
            g_numa_topology.nodes[0].cpus[i] = i;
        }
    } else {
        // NUMA is available
        g_numa_topology.numa_available = true;
        g_numa_topology.num_nodes = numa_num_configured_nodes();
        g_numa_topology.num_cpus = numa_num_configured_cpus();
        
        if (g_numa_topology.num_nodes > SEEN_NUMA_MAX_NODES) {
            fprintf(stderr, "[SEEN NUMA] Warning: System has %d nodes, max supported is %d\n",
                    g_numa_topology.num_nodes, SEEN_NUMA_MAX_NODES);
            g_numa_topology.num_nodes = SEEN_NUMA_MAX_NODES;
        }
        
        // Gather info for each node
        for (int node = 0; node < g_numa_topology.num_nodes; node++) {
            SeenNUMANodeInfo* info = &g_numa_topology.nodes[node];
            info->node_id = node;
            info->is_available = true;
            
            // Get memory info
            long free_mem = 0;
            long total_mem = numa_node_size(node, &free_mem);
            info->total_memory = (size_t)total_mem;
            info->free_memory = (size_t)free_mem;
            
            // Find CPUs assigned to this node
            info->num_cpus = 0;
            for (int cpu = 0; cpu < g_numa_topology.num_cpus && 
                            info->num_cpus < SEEN_NUMA_MAX_CPUS; cpu++) {
                if (numa_node_of_cpu(cpu) == node) {
                    info->cpus[info->num_cpus++] = cpu;
                }
            }
        }
    }
#else
    // No libnuma support - single node fallback
    g_numa_topology.numa_available = false;
    g_numa_topology.num_nodes = 1;
    g_numa_topology.num_cpus = (int)sysconf(_SC_NPROCESSORS_ONLN);
    g_numa_topology.nodes[0].node_id = 0;
    g_numa_topology.nodes[0].is_available = true;
    g_numa_topology.nodes[0].num_cpus = g_numa_topology.num_cpus;
    
    int max_cpus = g_numa_topology.num_cpus < SEEN_NUMA_MAX_CPUS 
                   ? g_numa_topology.num_cpus : SEEN_NUMA_MAX_CPUS;
    for (int i = 0; i < max_cpus; i++) {
        g_numa_topology.nodes[0].cpus[i] = i;
    }
#endif
    
    g_numa_topology.initialized = true;
    g_numa_initialized = 1;
}

const SeenNUMATopology* seen_numa_get_topology(void) {
    if (!g_numa_initialized) {
        seen_numa_init();
    }
    return &g_numa_topology;
}

bool seen_numa_is_available(void) {
    if (!g_numa_initialized) {
        seen_numa_init();
    }
    return g_numa_topology.numa_available;
}

int seen_numa_num_nodes(void) {
    if (!g_numa_initialized) {
        seen_numa_init();
    }
    return g_numa_topology.num_nodes;
}

int seen_numa_current_node(void) {
    if (!g_numa_initialized) {
        seen_numa_init();
    }
    
#ifdef SEEN_NUMA_USE_LIBNUMA
    // Get current CPU and find its NUMA node
    int cpu = sched_getcpu();
    if (cpu >= 0 && cpu < g_numa_topology.num_cpus) {
        return numa_node_of_cpu(cpu);
    }
#endif
    
    return 0; // Default to node 0
}

const SeenNUMANodeInfo* seen_numa_get_node_info(int node_id) {
    if (!g_numa_initialized) {
        seen_numa_init();
    }
    
    if (node_id < 0 || node_id >= g_numa_topology.num_nodes) {
        return NULL;
    }
    
    return &g_numa_topology.nodes[node_id];
}

void seen_numa_print_topology(void) {
    if (!g_numa_initialized) {
        seen_numa_init();
    }
    
    fprintf(stderr, "\n");
    fprintf(stderr, "╔══════════════════════════════════════════════════════════════════════╗\n");
    fprintf(stderr, "║                      SEEN NUMA TOPOLOGY                              ║\n");
    fprintf(stderr, "╠══════════════════════════════════════════════════════════════════════╣\n");
    
    if (!g_numa_topology.numa_available) {
        fprintf(stderr, "║ NUMA support: Not available (using single-node fallback)             ║\n");
    } else {
        fprintf(stderr, "║ NUMA support: Available (libnuma)                                    ║\n");
    }
    
    fprintf(stderr, "║ Total NUMA nodes: %-3d                                                ║\n", 
            g_numa_topology.num_nodes);
    fprintf(stderr, "║ Total CPUs: %-3d                                                      ║\n", 
            g_numa_topology.num_cpus);
    fprintf(stderr, "╠══════════════════════════════════════════════════════════════════════╣\n");
    
    for (int i = 0; i < g_numa_topology.num_nodes; i++) {
        const SeenNUMANodeInfo* info = &g_numa_topology.nodes[i];
        fprintf(stderr, "║ Node %d:                                                              ║\n", i);
        
        if (info->total_memory > 0) {
            double total_gb = info->total_memory / (1024.0 * 1024.0 * 1024.0);
            double free_gb = info->free_memory / (1024.0 * 1024.0 * 1024.0);
            fprintf(stderr, "║   Memory: %.2f GB total, %.2f GB free                               ║\n",
                    total_gb, free_gb);
        }
        
        fprintf(stderr, "║   CPUs: %d [", info->num_cpus);
        int cpus_to_show = info->num_cpus < 8 ? info->num_cpus : 8;
        for (int j = 0; j < cpus_to_show; j++) {
            fprintf(stderr, "%d", info->cpus[j]);
            if (j < cpus_to_show - 1) fprintf(stderr, ", ");
        }
        if (info->num_cpus > 8) {
            fprintf(stderr, ", ...+%d more", info->num_cpus - 8);
        }
        fprintf(stderr, "]\n");
        fprintf(stderr, "║                                                                      ║\n");
    }
    
    fprintf(stderr, "╚══════════════════════════════════════════════════════════════════════╝\n");
    fprintf(stderr, "\n");
}

// ============================================================================
// NUMA-Aware Memory Allocation
// ============================================================================

void* seen_numa_alloc_onnode(size_t size, int node) {
    if (!g_numa_initialized) {
        seen_numa_init();
    }
    
    if (size == 0) return NULL;
    
    // Validate node
    if (node < 0) {
        node = seen_numa_current_node();
    } else if (node >= g_numa_topology.num_nodes) {
        node = 0;
    }
    
    void* ptr = NULL;
    
#ifdef SEEN_NUMA_USE_LIBNUMA
    if (g_numa_topology.numa_available) {
        ptr = numa_alloc_onnode(size, node);
        if (!ptr) {
            // Fallback to malloc on failure
            ptr = malloc(size);
        }
    } else {
        ptr = malloc(size);
    }
#else
    ptr = malloc(size);
#endif
    
    if (ptr) {
        seen_numa_stat_alloc(node, size);
    }
    
    return ptr;
}

void* seen_numa_alloc_interleaved(size_t size) {
    if (!g_numa_initialized) {
        seen_numa_init();
    }
    
    if (size == 0) return NULL;
    
    void* ptr = NULL;
    
#ifdef SEEN_NUMA_USE_LIBNUMA
    if (g_numa_topology.numa_available) {
        ptr = numa_alloc_interleaved(size);
        if (!ptr) {
            ptr = malloc(size);
        }
    } else {
        ptr = malloc(size);
    }
#else
    ptr = malloc(size);
#endif
    
    if (ptr) {
        // Track as allocated on all nodes for stats purposes
        for (int i = 0; i < g_numa_topology.num_nodes; i++) {
            seen_numa_stat_alloc(i, size / g_numa_topology.num_nodes);
        }
    }
    
    return ptr;
}

void* seen_numa_alloc_local(size_t size) {
    return seen_numa_alloc_onnode(size, -1);
}

void* seen_numa_alloc_aligned(size_t size, size_t alignment, int node) {
    if (!g_numa_initialized) {
        seen_numa_init();
    }
    
    if (size == 0) return NULL;
    
    // Validate alignment (must be power of 2)
    if (alignment == 0 || (alignment & (alignment - 1)) != 0) {
        alignment = sizeof(void*);
    }
    
    void* ptr = NULL;
    
#ifdef SEEN_NUMA_USE_LIBNUMA
    if (g_numa_topology.numa_available) {
        // Use posix_memalign with NUMA awareness via mbind
        if (posix_memalign(&ptr, alignment, size) == 0) {
            // Bind to specific node if requested
            if (node >= 0 && node < g_numa_topology.num_nodes) {
                unsigned long nodemask = (1UL << node);
                mbind(ptr, size, MPOL_BIND, &nodemask, sizeof(nodemask) * 8, 0);
            }
        }
    } else {
        posix_memalign(&ptr, alignment, size);
    }
#else
    posix_memalign(&ptr, alignment, size);
#endif
    
    if (ptr) {
        if (node < 0) node = seen_numa_current_node();
        seen_numa_stat_alloc(node, size);
    }
    
    return ptr;
}

void seen_numa_free(void* ptr, size_t size) {
    if (!ptr) return;
    
    int node = seen_numa_get_node(ptr);
    
#ifdef SEEN_NUMA_USE_LIBNUMA
    if (g_numa_topology.numa_available) {
        numa_free(ptr, size);
    } else {
        free(ptr);
    }
#else
    free(ptr);
#endif
    
    seen_numa_stat_free(node, size);
}

void* seen_numa_realloc(void* ptr, size_t old_size, size_t new_size, int node) {
    if (!ptr) {
        return seen_numa_alloc_onnode(new_size, node);
    }
    
    if (new_size == 0) {
        seen_numa_free(ptr, old_size);
        return NULL;
    }
    
    // Allocate new memory and copy
    void* new_ptr = seen_numa_alloc_onnode(new_size, node);
    if (!new_ptr) return NULL;
    
    size_t copy_size = old_size < new_size ? old_size : new_size;
    memcpy(new_ptr, ptr, copy_size);
    
    seen_numa_free(ptr, old_size);
    
    return new_ptr;
}

int seen_numa_get_node(void* ptr) {
    if (!ptr) return -1;
    
#ifdef SEEN_NUMA_USE_LIBNUMA
    if (!g_numa_topology.numa_available) return 0;
    
    int status[1];
    void* pages[1] = { ptr };
    int ret = numa_move_pages(0, 1, pages, NULL, status, 0);
    
    if (ret == 0 && status[0] >= 0) {
        return status[0];
    }
#endif
    
    return -1;
}

int seen_numa_move_pages(void* ptr, size_t size, int target_node) {
    if (!ptr || size == 0) return -1;
    
    if (target_node < 0 || target_node >= g_numa_topology.num_nodes) {
        return -1;
    }
    
#ifdef SEEN_NUMA_USE_LIBNUMA
    if (!g_numa_topology.numa_available) return 0; // No-op on non-NUMA
    
    // Calculate number of pages
    long page_size = sysconf(_SC_PAGESIZE);
    int num_pages = (int)((size + page_size - 1) / page_size);
    
    if (num_pages <= 0) return 0;
    
    // Allocate arrays for move_pages
    void** pages = (void**)malloc(num_pages * sizeof(void*));
    int* nodes = (int*)malloc(num_pages * sizeof(int));
    int* status = (int*)malloc(num_pages * sizeof(int));
    
    if (!pages || !nodes || !status) {
        free(pages);
        free(nodes);
        free(status);
        return -1;
    }
    
    // Fill in page addresses and target nodes
    char* base = (char*)ptr;
    for (int i = 0; i < num_pages; i++) {
        pages[i] = base + (i * page_size);
        nodes[i] = target_node;
    }
    
    int ret = numa_move_pages(0, num_pages, pages, nodes, status, 0);
    
    free(pages);
    free(nodes);
    free(status);
    
    return ret;
#else
    (void)size;
    (void)target_node;
    return 0; // Success (no-op on non-NUMA systems)
#endif
}

// ============================================================================
// Thread Affinity Bindings
// ============================================================================

bool seen_numa_bind_thread_to_node(int node) {
    if (!g_numa_initialized) {
        seen_numa_init();
    }
    
    if (node < 0 || node >= g_numa_topology.num_nodes) {
        return false;
    }
    
#ifdef SEEN_NUMA_USE_LIBNUMA
    if (g_numa_topology.numa_available) {
        int ret = numa_run_on_node(node);
        return ret == 0;
    }
#endif
    
    (void)node;
    return true; // Success on non-NUMA (no-op)
}

bool seen_numa_bind_thread_to_cpus(const int* cpus, int num_cpus) {
    if (!cpus || num_cpus <= 0) return false;
    
#ifdef SEEN_NUMA_USE_LIBNUMA
    cpu_set_t cpuset;
    CPU_ZERO(&cpuset);
    
    for (int i = 0; i < num_cpus; i++) {
        if (cpus[i] >= 0 && cpus[i] < g_numa_topology.num_cpus) {
            CPU_SET(cpus[i], &cpuset);
        }
    }
    
    int ret = sched_setaffinity(0, sizeof(cpuset), &cpuset);
    return ret == 0;
#else
    (void)cpus;
    (void)num_cpus;
    return true;
#endif
}

int seen_numa_get_thread_affinity(int* cpus, int max_cpus) {
    if (!cpus || max_cpus <= 0) return -1;
    
#ifdef SEEN_NUMA_USE_LIBNUMA
    cpu_set_t cpuset;
    CPU_ZERO(&cpuset);
    
    if (sched_getaffinity(0, sizeof(cpuset), &cpuset) != 0) {
        return -1;
    }
    
    int count = 0;
    for (int i = 0; i < g_numa_topology.num_cpus && count < max_cpus; i++) {
        if (CPU_ISSET(i, &cpuset)) {
            cpus[count++] = i;
        }
    }
    
    return count;
#else
    // Return all CPUs on non-NUMA
    int count = max_cpus < g_numa_topology.num_cpus ? max_cpus : g_numa_topology.num_cpus;
    for (int i = 0; i < count; i++) {
        cpus[i] = i;
    }
    return count;
#endif
}

void seen_numa_run_on_node(int node, void (*func)(void*), void* arg) {
    if (!func) return;
    
#ifdef SEEN_NUMA_USE_LIBNUMA
    if (g_numa_topology.numa_available) {
        // Save current binding
        void* old_mask = numa_allocate_nodemask();
        if (!old_mask) {
            // Just run without changing affinity
            func(arg);
            return;
        }
        
        // Get current affinity (best effort)
        int old_node = seen_numa_current_node();
        
        // Bind to target node
        if (numa_run_on_node(node) == 0) {
            func(arg);
            // Restore original binding
            numa_run_on_node(old_node);
        } else {
            func(arg);
        }
    } else {
        func(arg);
    }
#else
    (void)node;
    func(arg);
#endif
}

// ============================================================================
// High-Level Memory Policy
// ============================================================================

void seen_numa_set_strategy(SeenNUMAStrategy strategy) {
    g_numa_strategy = strategy;
}

SeenNUMAStrategy seen_numa_get_strategy(void) {
    return g_numa_strategy;
}

void* seen_numa_alloc(size_t size) {
    switch (g_numa_strategy) {
        case SEEN_NUMA_STRATEGY_INTERLEAVED:
            return seen_numa_alloc_interleaved(size);
        case SEEN_NUMA_STRATEGY_BIND:
        case SEEN_NUMA_STRATEGY_PREFERRED:
            // Use current node for bind/preferred
            return seen_numa_alloc_local(size);
        case SEEN_NUMA_STRATEGY_LOCAL:
        default:
            return seen_numa_alloc_local(size);
    }
}

void* seen_numa_alloc_aligned_strategy(size_t size, size_t alignment) {
    switch (g_numa_strategy) {
        case SEEN_NUMA_STRATEGY_INTERLEAVED:
            // Interleaved doesn't support aligned allocation well
            return seen_numa_alloc_aligned(size, alignment, -1);
        case SEEN_NUMA_STRATEGY_BIND:
        case SEEN_NUMA_STRATEGY_PREFERRED:
        case SEEN_NUMA_STRATEGY_LOCAL:
        default:
            return seen_numa_alloc_aligned(size, alignment, -1);
    }
}

// ============================================================================
// Batch/Multi-Page Operations
// ============================================================================

int seen_numa_alloc_pages(void** pages, int num_pages, size_t page_size, const int* nodes) {
    if (!pages || num_pages <= 0 || page_size == 0) return 0;
    
    int allocated = 0;
    
    for (int i = 0; i < num_pages; i++) {
        int node = nodes ? nodes[i] : -1;
        pages[i] = seen_numa_alloc_onnode(page_size, node);
        if (pages[i]) {
            allocated++;
        } else {
            // Stop on first failure
            break;
        }
    }
    
    return allocated;
}

void seen_numa_free_pages(void** pages, int num_pages, size_t page_size) {
    if (!pages || num_pages <= 0) return;
    
    for (int i = 0; i < num_pages; i++) {
        if (pages[i]) {
            seen_numa_free(pages[i], page_size);
            pages[i] = NULL;
        }
    }
}

// ============================================================================
// Performance Monitoring
// ============================================================================

bool seen_numa_get_node_stats(int node_id, SeenNUMANodeStats* stats) {
    if (!stats) return false;
    if (node_id < 0 || node_id >= SEEN_NUMA_MAX_NODES) return false;
    
    memcpy(stats, &g_numa_stats[node_id], sizeof(SeenNUMANodeStats));
    return true;
}

void seen_numa_reset_stats(void) {
    memset(g_numa_stats, 0, sizeof(g_numa_stats));
}

void seen_numa_print_stats(void) {
    fprintf(stderr, "\n");
    fprintf(stderr, "╔══════════════════════════════════════════════════════════════════════╗\n");
    fprintf(stderr, "║                      SEEN NUMA STATISTICS                            ║\n");
    fprintf(stderr, "╠══════════════════════════════════════════════════════════════════════╣\n");
    fprintf(stderr, "║ %-6s %12s %12s %12s %12s ║\n", 
            "Node", "Total(GB)", "Active(GB)", "Allocs", "Frees");
    fprintf(stderr, "╠══════════════════════════════════════════════════════════════════════╣\n");
    
    size_t total_allocated = 0;
    size_t total_active = 0;
    
    for (int i = 0; i < g_numa_topology.num_nodes; i++) {
        SeenNUMANodeStats* stats = &g_numa_stats[i];
        double total_gb = stats->total_bytes / (1024.0 * 1024.0 * 1024.0);
        double active_gb = stats->active_bytes / (1024.0 * 1024.0 * 1024.0);
        
        fprintf(stderr, "║ %-6d %12.3f %12.3f %12zu %12zu ║\n",
                i, total_gb, active_gb, 
                (size_t)stats->allocation_count, 
                (size_t)stats->free_count);
        
        total_allocated += stats->total_bytes;
        total_active += stats->active_bytes;
    }
    
    fprintf(stderr, "╠══════════════════════════════════════════════════════════════════════╣\n");
    fprintf(stderr, "║ %-6s %12.3f %12.3f %35s ║\n", 
            "Total", 
            total_allocated / (1024.0 * 1024.0 * 1024.0),
            total_active / (1024.0 * 1024.0 * 1024.0),
            "");
    fprintf(stderr, "╚══════════════════════════════════════════════════════════════════════╝\n");
    fprintf(stderr, "\n");
}

// ============================================================================
// Integration with Seen Runtime
// ============================================================================

void seen_numa_runtime_init(void) {
    seen_numa_init();
    
    // Print topology info in debug mode
    const char* debug_env = getenv("SEEN_NUMA_DEBUG");
    if (debug_env && debug_env[0] == '1') {
        seen_numa_print_topology();
    }
}

int seen_numa_recommended_node(void) {
    if (!g_numa_initialized) {
        seen_numa_init();
    }
    
    int num_nodes = g_numa_topology.num_nodes;
    if (num_nodes <= 1) return 0;
    
    // Simple round-robin for load balancing
    int node = atomic_fetch_add(&g_numa_round_robin, 1) % num_nodes;
    return node;
}

void* seen_numa_array_alloc(int64_t element_size, int64_t capacity, int node) {
    if (element_size <= 0 || capacity <= 0) return NULL;
    
    size_t size = (size_t)(element_size * capacity);
    void* ptr = seen_numa_alloc_onnode(size, node);
    return ptr;
}

// Auto-initialize on first use if constructors are supported
#if defined(__GNUC__) || defined(__clang__)
__attribute__((constructor))
static void seen_numa_auto_init(void) {
    // Don't auto-init to avoid side effects
    // User should call seen_numa_init() explicitly
}
#endif
