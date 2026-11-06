/* TSan test coordination only; all conflicting counter accesses stay in Seen. */
#include <pthread.h>
#include <stdatomic.h>
#include <stdint.h>

/* LLVM 20's own invisible test barrier preserves access trace history without
 * adding happens-before edges. Its tsan_interceptors_posix.cpp documents why
 * instrumented atomic-plus-yield spinning can lose race reports. Do not replace
 * this with sanitizer exclusions, annotations, or a helping Seen barrier. */
extern void __tsan_testonly_barrier_init(void *barrier, uint32_t threads);
extern void __tsan_testonly_barrier_wait(void *barrier);

_Static_assert(ATOMIC_INT_LOCK_FREE == 2, "worker control requires lock-free atomics");
static uint32_t barriers[2];
static _Atomic unsigned arrivals[2];
static pthread_t participants[2];

/* Called before worker creation; spawn publishes the initialized barriers. */
void seen_tsan_workers_init(void) {
    __tsan_testonly_barrier_init(&barriers[0], 2);
    __tsan_testonly_barrier_init(&barriers[1], 2);
}

int32_t seen_tsan_workers_phase(uint32_t phase) {
    if (phase >= 2) return 64;
    unsigned previous = atomic_fetch_add_explicit(&arrivals[phase], 1, memory_order_relaxed);
    if (previous >= 2) return 65;
    if (phase == 0) participants[previous] = pthread_self();
    __tsan_testonly_barrier_wait(&barriers[phase]);
    return phase == 0 ? (int32_t)previous : 0;
}

/* Called only after both job joins, which publish the participant identities. */
int32_t seen_tsan_workers_distinct_threads(void) {
    return atomic_load_explicit(&arrivals[0], memory_order_relaxed) == 2 &&
           !pthread_equal(participants[0], participants[1]);
}
