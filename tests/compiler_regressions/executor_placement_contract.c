#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include "src/executor_abi.h"
#include "sync_abi.h"
#include <assert.h>
#include <errno.h>
#include <sched.h>
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include <sys/syscall.h>

static int affinity_calls, memory_calls, deny_affinity, deny_memory;
int __wrap_sched_getaffinity(pid_t pid, size_t size, cpu_set_t *set) {
    assert(pid == 0 && size == sizeof(*set));
    CPU_ZERO(set); CPU_SET(2, set);
    return 0;
}
int __wrap_sched_setaffinity(pid_t pid, size_t size, const cpu_set_t *set) {
    assert(pid == 0 && size == sizeof(*set));
    affinity_calls++;
    if (deny_affinity || !CPU_ISSET(2, set)) { errno = EINVAL; return -1; }
    return 0;
}
long __wrap_syscall(long operation, ...) {
    va_list arguments; va_start(arguments, operation);
    if (operation == SYS_get_mempolicy) {
        (void)va_arg(arguments, void *);
        unsigned long *mask = va_arg(arguments, unsigned long *);
        *mask = 2UL;
    } else {
        assert(operation == SYS_set_mempolicy);
        (void)va_arg(arguments, int);
        unsigned long *mask = va_arg(arguments, unsigned long *);
        assert(*mask == 2UL);
        memory_calls++;
    }
    va_end(arguments);
    if (deny_memory) { errno = deny_memory; return -1; }
    return 0;
}
int main(void) {
    assert(seen_executor_validate_placement(-1, -1) == SEEN_SYNC_OK);
    assert(seen_executor_apply_placement(-1, -1) == SEEN_SYNC_OK);
    assert(affinity_calls == 0 && memory_calls == 0);
    assert(seen_executor_validate_placement(-2, -1) == SEEN_SYNC_INVALID);
    assert(seen_executor_apply_placement(-1, -2) == SEEN_SYNC_INVALID);
    assert(seen_executor_validate_placement(1024, -1) == SEEN_SYNC_UNSUPPORTED);
    assert(seen_executor_apply_placement(-1, 64) == SEEN_SYNC_UNSUPPORTED);
    assert(seen_executor_validate_placement(2, 1) == SEEN_SYNC_OK);
    assert(seen_executor_validate_placement(3, -1) == SEEN_SYNC_UNAVAILABLE);
    assert(seen_executor_validate_placement(-1, 0) == SEEN_SYNC_UNAVAILABLE);
    assert(seen_executor_apply_placement(2, 1) == SEEN_SYNC_OK);
    assert(affinity_calls == 1 && memory_calls == 1);
    deny_affinity = 1;
    assert(seen_executor_apply_placement(2, -1) == SEEN_SYNC_UNAVAILABLE);
    deny_memory = EPERM;
    assert(seen_executor_validate_placement(-1, 1) == SEEN_SYNC_UNAVAILABLE);
    assert(seen_executor_apply_placement(-1, 1) == SEEN_SYNC_UNAVAILABLE);
    deny_memory = ENOSYS;
    assert(seen_executor_apply_placement(-1, 1) == SEEN_SYNC_UNSUPPORTED);
    puts("PASS: explicit placement ABI geometry, permission errors and unsupported calls");
    return 0;
}
