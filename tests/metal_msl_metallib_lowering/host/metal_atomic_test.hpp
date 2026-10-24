// Test-only CPU normalization of MSL atomic word pointers. Raw word storage
// uses CPU atomic builtins; it is never cast to an unconstructed std::atomic.
// This does not model a GPU, its execution barriers or its memory scopes.
#pragma once
#include <metal_stdlib>
#include <cstdlib>
namespace metal {
using std::memory_order;
using std::memory_order_acquire;
using std::memory_order_release;
using std::memory_order_acq_rel;
using std::memory_order_seq_cst;
inline uint seen_cas_spurious_remaining = 0;
inline uint seen_cas_calls = 0;
inline void seen_require_relaxed(memory_order order) {
    if (order != memory_order_relaxed) std::abort();
}
template<class T> T atomic_load_explicit(const T* object, memory_order order) {
    static_assert(std::is_integral_v<T> && sizeof(T) == 4);
    seen_require_relaxed(order); return __atomic_load_n(object, __ATOMIC_RELAXED);
}
template<class T> void atomic_store_explicit(T* object, T desired, memory_order order) {
    static_assert(std::is_integral_v<T> && sizeof(T) == 4);
    seen_require_relaxed(order); __atomic_store_n(object, desired, __ATOMIC_RELAXED);
}
template<class T> T atomic_exchange_explicit(T* object, T desired, memory_order order) {
    static_assert(std::is_integral_v<T> && sizeof(T) == 4);
    seen_require_relaxed(order); return __atomic_exchange_n(object, desired, __ATOMIC_RELAXED);
}
#define SEEN_TEST_ATOMIC(name, builtin) \
template<class T> T name(T* object, T desired, memory_order order) { \
    static_assert(std::is_integral_v<T> && sizeof(T) == 4); \
    seen_require_relaxed(order); return builtin(object, desired, __ATOMIC_RELAXED); \
}
SEEN_TEST_ATOMIC(atomic_fetch_add_explicit, __atomic_fetch_add)
SEEN_TEST_ATOMIC(atomic_fetch_sub_explicit, __atomic_fetch_sub)
SEEN_TEST_ATOMIC(atomic_fetch_and_explicit, __atomic_fetch_and)
SEEN_TEST_ATOMIC(atomic_fetch_or_explicit, __atomic_fetch_or)
SEEN_TEST_ATOMIC(atomic_fetch_xor_explicit, __atomic_fetch_xor)
#undef SEEN_TEST_ATOMIC
template<class T> T seen_atomic_extremum(T* object, T desired, memory_order order, bool maximum) {
    static_assert(std::is_integral_v<T> && sizeof(T) == 4);
    seen_require_relaxed(order); T old = __atomic_load_n(object, __ATOMIC_RELAXED);
    for (uint attempt = 0; attempt < 1000000u; ++attempt) {
        T replacement = maximum ? (old > desired ? old : desired) : (old < desired ? old : desired);
        if (__atomic_compare_exchange_n(object, &old, replacement, true, __ATOMIC_RELAXED, __ATOMIC_RELAXED)) return old;
    }
    std::abort();
}
template<class T> T atomic_fetch_min_explicit(T* object, T desired, memory_order order) {
    return seen_atomic_extremum(object, desired, order, false);
}
template<class T> T atomic_fetch_max_explicit(T* object, T desired, memory_order order) {
    return seen_atomic_extremum(object, desired, order, true);
}
template<class T> bool atomic_compare_exchange_weak_explicit(T* object, T* expected, T desired,
                                                           memory_order success, memory_order failure) {
    static_assert(std::is_integral_v<T> && sizeof(T) == 4);
    seen_require_relaxed(success); seen_require_relaxed(failure); ++seen_cas_calls;
    if (seen_cas_spurious_remaining != 0) {
        --seen_cas_spurious_remaining; *expected = __atomic_load_n(object, __ATOMIC_RELAXED); return false;
    }
    return __atomic_compare_exchange_n(object, expected, desired, true, __ATOMIC_RELAXED, __ATOMIC_RELAXED);
}
enum thread_scope { thread_scope_thread = 1, thread_scope_simdgroup, thread_scope_threadgroup, thread_scope_device };
struct SeenFenceEvent { int spaces; memory_order order; thread_scope scope; };
inline SeenFenceEvent seen_fence_events[256];
inline uint seen_fence_count = 0;
inline void atomic_thread_fence(int spaces, memory_order order, thread_scope scope) {
    if (seen_fence_count >= 256) std::abort();
    seen_fence_events[seen_fence_count++] = {spaces, order, scope}; std::atomic_thread_fence(order);
}
inline void simdgroup_barrier(int) {}
}
