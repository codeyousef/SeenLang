/* Coordination only: the conflicting counter reads/stores remain in Seen.
 * Distinct phases avoid allocator and handle-registry locks accidentally
 * ordering the writes while one worker is still leaving its first barrier. */
#include <sched.h>
#include <stdatomic.h>
#include <stdint.h>

_Static_assert(ATOMIC_INT_LOCK_FREE == 2, "worker control requires lock-free atomics");
static _Atomic unsigned arrivals[2];

int32_t seen_tsan_workers_phase(uint32_t phase) {
    if (phase >= 2) return 64;
    if (atomic_fetch_add_explicit(&arrivals[phase], 1, memory_order_acq_rel) >= 2)
        return 65;
    for (unsigned spins = 0; spins < 10000000; ++spins) {
        if (atomic_load_explicit(&arrivals[phase], memory_order_acquire) == 2)
            return 0;
        (void)sched_yield();
    }
    return 66;
}
