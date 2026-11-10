#if defined(__linux__) && !defined(_GNU_SOURCE)
#define _GNU_SOURCE
#endif
#if !defined(_WIN32) && !defined(_POSIX_C_SOURCE)
#define _POSIX_C_SOURCE 200809L
#endif

#if defined(__APPLE__) && !defined(_DARWIN_C_SOURCE)
#define _DARWIN_C_SOURCE
#endif
#include "executor_abi.h"
#include "../sync_abi.h"

#if ((defined(__linux__) && defined(__x86_64__)) || \
     (defined(__APPLE__) && (defined(__x86_64__) || defined(__aarch64__)))) && \
    !defined(SEEN_EXECUTOR_FORCE_UNSUPPORTED)
#include <errno.h>
#include <limits.h>
#include <pthread.h>
#include <stdatomic.h>
#include <stddef.h>
#include <unistd.h>
#if defined(__linux__)
#include <sched.h>
#include <sys/syscall.h>
#include <linux/mempolicy.h>
#endif

/* Static storage prevents allocation failure from losing a joinable thread.
 * Slots stay occupied until a successful join, including after thread exit. */
#ifndef SEEN_EXECUTOR_THREAD_SLOTS
#define SEEN_EXECUTOR_THREAD_SLOTS 256U
#endif
#define SEEN_EXECUTOR_MIN_STACK UINT64_C(65536)
#define SEEN_EXECUTOR_MAX_STACK UINT64_C(8388608)

typedef struct SeenExecutorThreadSlot {
    pthread_t thread;
    SeenExecutorEntry entry;
    int64_t argument;
    uint32_t generation;
    int occupied;
    int joining;
} SeenExecutorThreadSlot;

static SeenExecutorThreadSlot seen_executor_threads[SEEN_EXECUTOR_THREAD_SLOTS];
static pthread_mutex_t seen_executor_registry = PTHREAD_MUTEX_INITIALIZER;
static _Atomic uint64_t seen_executor_thread_count;
static _Thread_local uint64_t seen_executor_owner;
static _Thread_local int64_t seen_executor_task;

static void *seen_executor_os_entry(void *raw) {
    SeenExecutorThreadSlot *slot = raw;
    /* The slot is immutable between successful create and successful join. */
    seen_executor_owner = (uint64_t)slot->argument;
    (void)slot->entry(slot->argument);
    seen_executor_owner = 0;
    return NULL;
}

static uint64_t seen_executor_thread_handle(uint32_t index,
                                           uint32_t generation) {
    return ((uint64_t)generation << 32) | ((uint64_t)index + 1);
}

int32_t seen_executor_thread_create(int64_t entry, int64_t argument,
    uint64_t stack_bytes, uint64_t *out_handle) {
    if (!out_handle || *out_handle != 0 || entry == 0 || argument == 0)
        return SEEN_SYNC_INVALID;
    if (stack_bytes < SEEN_EXECUTOR_MIN_STACK ||
        stack_bytes > SEEN_EXECUTOR_MAX_STACK || stack_bytes > SIZE_MAX)
        return SEEN_SYNC_LIMIT;

    pthread_attr_t attr;
    if (pthread_attr_init(&attr) != 0) return SEEN_SYNC_UNAVAILABLE;
    if (pthread_attr_setstacksize(&attr, (size_t)stack_bytes) != 0) {
        pthread_attr_destroy(&attr);
        return SEEN_SYNC_INVALID;
    }
    if (pthread_mutex_lock(&seen_executor_registry) != 0) {
        pthread_attr_destroy(&attr);
        return SEEN_SYNC_UNAVAILABLE;
    }
    uint32_t index;
    for (index = 0; index < SEEN_EXECUTOR_THREAD_SLOTS; ++index) {
        if (!seen_executor_threads[index].occupied &&
            seen_executor_threads[index].generation != UINT32_MAX) break;
    }
    if (index == SEEN_EXECUTOR_THREAD_SLOTS) {
        pthread_mutex_unlock(&seen_executor_registry);
        pthread_attr_destroy(&attr);
        return SEEN_SYNC_LIMIT;
    }
    SeenExecutorThreadSlot *slot = &seen_executor_threads[index];
    ++slot->generation;
    slot->entry = (SeenExecutorEntry)(uintptr_t)entry;
    slot->argument = argument;
    slot->occupied = 1;
    slot->joining = 0;
    int result = pthread_create(&slot->thread, &attr, seen_executor_os_entry,
                                slot);
    pthread_attr_destroy(&attr);
    if (result != 0) {
        slot->occupied = 0;
        slot->entry = NULL;
        slot->argument = 0;
        pthread_mutex_unlock(&seen_executor_registry);
        return result == EAGAIN ? SEEN_SYNC_LIMIT : SEEN_SYNC_UNAVAILABLE;
    }
    *out_handle = seen_executor_thread_handle(index, slot->generation);
    atomic_fetch_add_explicit(&seen_executor_thread_count, 1,
                               memory_order_relaxed);
    pthread_mutex_unlock(&seen_executor_registry);
    return SEEN_SYNC_OK;
}

int32_t seen_executor_thread_join(uint64_t *handle) {
    if (!handle) return SEEN_SYNC_INVALID;
    if (*handle == 0) return SEEN_SYNC_OK;
    uint64_t value = *handle;
    uint32_t low = (uint32_t)value;
    uint32_t generation = (uint32_t)(value >> 32);
    if (low == 0 || low > SEEN_EXECUTOR_THREAD_SLOTS || generation == 0)
        return SEEN_SYNC_INVALID;
    if (pthread_mutex_lock(&seen_executor_registry) != 0)
        return SEEN_SYNC_UNAVAILABLE;
    SeenExecutorThreadSlot *slot = &seen_executor_threads[low - 1];
    if (!slot->occupied || slot->generation != generation ||
        pthread_equal(slot->thread, pthread_self())) {
        pthread_mutex_unlock(&seen_executor_registry);
        return SEEN_SYNC_INVALID;
    }
    if (slot->joining) {
        pthread_mutex_unlock(&seen_executor_registry);
        return SEEN_SYNC_BUSY;
    }
    slot->joining = 1;
    pthread_t thread = slot->thread;
    pthread_mutex_unlock(&seen_executor_registry);
    int result = pthread_join(thread, NULL);
    pthread_mutex_lock(&seen_executor_registry);
    if (result != 0) {
        /* Preserve the sole joinable owner for explicit retry. */
        slot->joining = 0;
        pthread_mutex_unlock(&seen_executor_registry);
        return SEEN_SYNC_UNAVAILABLE;
    }
    slot->entry = NULL;
    slot->argument = 0;
    slot->occupied = 0;
    slot->joining = 0;
    *handle = 0;
    atomic_fetch_sub_explicit(&seen_executor_thread_count, 1,
                               memory_order_relaxed);
    pthread_mutex_unlock(&seen_executor_registry);
    return SEEN_SYNC_OK;
}

#if defined(__linux__)
/* Fixed ABI geometry only. Selection/fallback and resource budgets are Seen. */
static int32_t seen_executor_placement_bounds(int32_t cpu, int32_t node) {
    if (cpu < -1 || node < -1) return SEEN_SYNC_INVALID;
    if (cpu >= CPU_SETSIZE || node >= (int32_t)(sizeof(unsigned long) * CHAR_BIT))
        return SEEN_SYNC_UNSUPPORTED;
    return SEEN_SYNC_OK;
}

int32_t seen_executor_validate_placement(int32_t cpu, int32_t node) {
    int32_t checked = seen_executor_placement_bounds(cpu, node);
    if (checked != SEEN_SYNC_OK) return checked;
    if (cpu >= 0) {
        cpu_set_t allowed;
        if (sched_getaffinity(0, sizeof(allowed), &allowed) != 0)
            return SEEN_SYNC_UNAVAILABLE;
        if (!CPU_ISSET(cpu, &allowed)) return SEEN_SYNC_UNAVAILABLE;
    }
    if (node >= 0) {
#if defined(SYS_get_mempolicy)
        unsigned long allowed = 0;
        if (syscall(SYS_get_mempolicy, NULL, &allowed,
                    sizeof(allowed) * CHAR_BIT, NULL, MPOL_F_MEMS_ALLOWED) != 0)
            return errno == ENOSYS ? SEEN_SYNC_UNSUPPORTED : SEEN_SYNC_UNAVAILABLE;
        if (!(allowed & (1UL << (unsigned)node))) return SEEN_SYNC_UNAVAILABLE;
#else
        return SEEN_SYNC_UNSUPPORTED;
#endif
    }
    return SEEN_SYNC_OK;
}

int32_t seen_executor_apply_placement(int32_t cpu, int32_t node) {
    int32_t checked = seen_executor_placement_bounds(cpu, node);
    if (checked != SEEN_SYNC_OK) return checked;
    if (node >= 0) {
#if defined(SYS_set_mempolicy)
        unsigned long mask = 1UL << (unsigned)node;
        if (syscall(SYS_set_mempolicy, MPOL_BIND, &mask,
                    sizeof(mask) * CHAR_BIT) != 0)
            return errno == ENOSYS ? SEEN_SYNC_UNSUPPORTED : SEEN_SYNC_UNAVAILABLE;
#else
        return SEEN_SYNC_UNSUPPORTED;
#endif
    }
    if (cpu >= 0) {
        cpu_set_t mask;
        CPU_ZERO(&mask);
        CPU_SET(cpu, &mask);
        /* The kernel enforces the current process cpuset. The caller may
         * deliberately change a worker's previous task affinity. */
        if (sched_setaffinity(0, sizeof(mask), &mask) != 0)
            return SEEN_SYNC_UNAVAILABLE;
    }
    return SEEN_SYNC_OK;
}

#else
int32_t seen_executor_validate_placement(int32_t cpu, int32_t node) {
    if (cpu < -1 || node < -1) return SEEN_SYNC_INVALID;
    return cpu == -1 && node == -1 ? SEEN_SYNC_OK : SEEN_SYNC_UNSUPPORTED;
}
int32_t seen_executor_apply_placement(int32_t cpu, int32_t node) {
    return seen_executor_validate_placement(cpu, node);
}
#endif

uint64_t seen_executor_current_owner(void) { return seen_executor_owner; }
int64_t seen_executor_current_task(void) { return seen_executor_task; }
int64_t seen_executor_swap_task(int64_t task) {
    int64_t previous = seen_executor_task;
    seen_executor_task = task;
    return previous;
}
uint64_t seen_executor_live_threads(void) {
    return atomic_load_explicit(&seen_executor_thread_count,
                                memory_order_relaxed);
}

#else
int32_t seen_executor_validate_placement(int32_t cpu, int32_t node) {
    if (cpu < -1 || node < -1) return SEEN_SYNC_INVALID;
    return cpu == -1 && node == -1 ? SEEN_SYNC_OK : SEEN_SYNC_UNSUPPORTED;
}
int32_t seen_executor_apply_placement(int32_t cpu, int32_t node) {
    return seen_executor_validate_placement(cpu, node);
}
int32_t seen_executor_thread_create(int64_t entry, int64_t argument,
    uint64_t stack_bytes, uint64_t *out_handle) {
    (void)entry; (void)argument; (void)stack_bytes; (void)out_handle;
    return SEEN_SYNC_UNSUPPORTED;
}
int32_t seen_executor_thread_join(uint64_t *handle) {
    if (handle && *handle == 0) return SEEN_SYNC_OK;
    return SEEN_SYNC_UNSUPPORTED;
}
uint64_t seen_executor_current_owner(void) { return 0; }
int64_t seen_executor_current_task(void) { return 0; }
int64_t seen_executor_swap_task(int64_t task) { (void)task; return 0; }
uint64_t seen_executor_live_threads(void) { return 0; }
#endif
