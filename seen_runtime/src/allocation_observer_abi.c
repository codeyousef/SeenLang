/* Allocation notifications only. The Seen owner decides which allocations it
 * retains and when to release them. Callbacks cannot observe their own storage.
 * This state is private to the calling thread and disabled by default. */
typedef int64_t (*SeenAllocationObserver)(void *, int64_t, uint64_t,
                                         uint64_t, int64_t, int64_t);
static _Thread_local SeenAllocationObserver seen_allocation_observer;
static _Thread_local void *seen_allocation_observer_context;
static _Thread_local unsigned seen_allocation_observer_busy;
static _Thread_local unsigned seen_allocation_observer_nested;
static _Thread_local unsigned seen_allocation_observer_reallocating;

int64_t seen_allocation_observer_install(void *context, uint64_t entry) {
    if (!context || !entry || seen_allocation_observer) return 0;
    seen_allocation_observer_context = context;
    seen_allocation_observer = (SeenAllocationObserver)(uintptr_t)entry;
    return 1;
}

int64_t seen_allocation_observer_remove(void *context) {
    if (!seen_allocation_observer || context != seen_allocation_observer_context ||
        seen_allocation_observer_busy || seen_allocation_observer_nested) return 0;
    seen_allocation_observer = NULL;
    seen_allocation_observer_context = NULL;
    return 1;
}

static int64_t seen_allocation_observe(int64_t event, uint64_t old,
                                      void *current, int64_t bytes, int64_t kind) {
    if (!seen_allocation_observer || seen_allocation_observer_busy ||
        (event == 3 && seen_allocation_observer_reallocating) ||
        (event != 3 && event != 4 && seen_allocation_observer_nested)) return 0;
    seen_allocation_observer_busy++;
    int64_t result = seen_allocation_observer(seen_allocation_observer_context,
        event, old, (uint64_t)(uintptr_t)current, bytes, kind);
    seen_allocation_observer_busy--;
    return result;
}

static void seen_observed_free(void *pointer) {
    if (pointer) seen_allocation_observe(3, (uint64_t)(uintptr_t)pointer, NULL, 0, 0);
    free(pointer);
}

#define free(pointer) seen_observed_free(pointer)

#ifdef _WIN32
static void seen_observed_aligned_free(void *pointer) {
    if (pointer) seen_allocation_observe(3, (uint64_t)(uintptr_t)pointer, NULL, 0, 0);
    _aligned_free(pointer);
}
#undef seen_aligned_free
#define seen_aligned_free(pointer) seen_observed_aligned_free(pointer)
#endif
