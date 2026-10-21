#if defined(__linux__) && !defined(_GNU_SOURCE)
#define _GNU_SOURCE
#endif
#include "io_uring_abi.h"
#include <stddef.h>
#include <stdatomic.h>
#if defined(__linux__) && (defined(__x86_64__) || defined(__aarch64__)) && \
    __BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__
#include <errno.h>
#include <limits.h>
#include <linux/io_uring.h>
#include <sys/eventfd.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <unistd.h>
_Static_assert(sizeof(struct io_uring_params) == 120, "Seen io_uring parameters ABI");
_Static_assert(offsetof(struct io_uring_params, sq_off) == 40, "Seen SQ offsets ABI");
_Static_assert(offsetof(struct io_uring_params, cq_off) == 80, "Seen CQ offsets ABI");
_Static_assert(sizeof(struct io_uring_sqe) == 64, "Seen SQ entry ABI");
_Static_assert(sizeof(struct io_uring_cqe) == 16, "Seen CQ entry ABI");
_Static_assert(offsetof(struct io_uring_sqe, user_data) == 32, "Seen SQ token ABI");
_Static_assert(IORING_OP_READ == 22 && IORING_OP_WRITE == 23 &&
    IORING_OP_ASYNC_CANCEL == 14, "Seen operation ABI");
static int seen_uring_fd_valid(int64_t value) { return value >= 0 && value <= INT_MAX; }
int64_t seen_uring_setup(uint32_t entries, void *parameters, uint64_t bytes) {
    if (!parameters || bytes != sizeof(struct io_uring_params)) return -EINVAL;
    long result = syscall(SYS_io_uring_setup, entries, parameters);
    return result < 0 ? -(int64_t)errno : (int64_t)result;
}
int64_t seen_uring_enter(int64_t ring, uint32_t submitted) {
    if (!seen_uring_fd_valid(ring)) return -EBADF;
    long result = syscall(SYS_io_uring_enter, (int)ring, submitted, 0U, 0U, NULL, 0U);
    return result < 0 ? -(int64_t)errno : (int64_t)result;
}
int64_t seen_uring_register(int64_t ring, uint32_t opcode, const void *argument, uint32_t count) {
    if (!seen_uring_fd_valid(ring)) return -EBADF;
    long result = syscall(SYS_io_uring_register, (int)ring, opcode, argument, count);
    return result < 0 ? -(int64_t)errno : (int64_t)result;
}
int64_t seen_uring_map(int64_t ring, uint64_t offset, uint64_t bytes, uint64_t *address) {
    if (!seen_uring_fd_valid(ring) || !address || !bytes || bytes > SIZE_MAX || offset > INT64_MAX)
        return -EINVAL;
    void *mapped = mmap(NULL, (size_t)bytes, PROT_READ | PROT_WRITE, MAP_SHARED, (int)ring, (off_t)offset);
    if (mapped == MAP_FAILED) return -(int64_t)errno;
    *address = (uint64_t)(uintptr_t)mapped;
    return 0;
}
int64_t seen_uring_unmap(void *address, uint64_t bytes) {
    if (!address || !bytes || bytes > SIZE_MAX) return -EINVAL;
    return munmap(address, (size_t)bytes) == 0 ? 0 : -(int64_t)errno;
}
int64_t seen_uring_eventfd_create(void) {
    int result = eventfd(0, EFD_CLOEXEC | EFD_NONBLOCK);
    return result < 0 ? -(int64_t)errno : (int64_t)result;
}
int64_t seen_uring_page_size(void) {
    long result = sysconf(_SC_PAGESIZE);
    return result > 0 ? (int64_t)result : -EIO;
}
int64_t seen_uring_eventfd_drain(int64_t descriptor) {
    if (!seen_uring_fd_valid(descriptor)) return -EBADF;
    uint64_t value = 0;
    ssize_t result = read((int)descriptor, &value, sizeof(value));
    if (result == sizeof(value) || (result < 0 && errno == EAGAIN)) return 0;
    return result < 0 ? -(int64_t)errno : -EIO;
}
#else
int64_t seen_uring_setup(uint32_t e, void *p, uint64_t b) { (void)e;(void)p;(void)b;return -38; }
int64_t seen_uring_enter(int64_t r, uint32_t n) { (void)r;(void)n;return -38; }
int64_t seen_uring_register(int64_t r,uint32_t o,const void*a,uint32_t n) {(void)r;(void)o;(void)a;(void)n;return -38;}
int64_t seen_uring_map(int64_t r,uint64_t o,uint64_t b,uint64_t*a) {(void)r;(void)o;(void)b;(void)a;return -38;}
int64_t seen_uring_unmap(void *a,uint64_t b) {(void)a;(void)b;return -38;}
int64_t seen_uring_eventfd_create(void) { return -38; }
int64_t seen_uring_page_size(void) { return -38; }
int64_t seen_uring_eventfd_drain(int64_t d) { (void)d;return -38; }
#endif
uint32_t seen_uring_load_acquire(const uint32_t *address) {
    return atomic_load_explicit((const _Atomic uint32_t *)address, memory_order_acquire);
}
void seen_uring_store_release(uint32_t *address, uint32_t value) {
    atomic_store_explicit((_Atomic uint32_t *)address, value, memory_order_release);
}
