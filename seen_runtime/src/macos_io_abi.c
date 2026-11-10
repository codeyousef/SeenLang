#if defined(__APPLE__) && !defined(_DARWIN_C_SOURCE)
#define _DARWIN_C_SOURCE
#endif
#include "macos_io_abi.h"
#if defined(__APPLE__) && (defined(__x86_64__) || defined(__aarch64__))
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <sys/stat.h>
#include <unistd.h>
_Static_assert(O_CLOEXEC == 0x01000000 && O_NONBLOCK == 4 && O_NOFOLLOW == 0x100,
    "Seen Darwin open flags ABI");
_Static_assert(O_CREAT == 0x200 && O_TRUNC == 0x400 && O_EXCL == 0x800 && O_APPEND == 8,
    "Seen Darwin creation flags ABI");
_Static_assert(S_IFMT == 0xf000 && S_IFREG == 0x8000, "Seen Darwin file mode ABI");
uint32_t seen_macos_io_abi(void) { return 1; }
int64_t seen_macos_io_open(const uint8_t *path, uint32_t flags, uint32_t mode) {
    if (!path) return -EINVAL;
    int result = open((const char *)path, (int)flags, (mode_t)mode);
    return result < 0 ? -(int64_t)errno : (int64_t)result;
}
int64_t seen_macos_io_mode(int64_t descriptor) {
    if (descriptor < 0 || descriptor > INT_MAX) return -EBADF;
    struct stat value;
    if (fstat((int)descriptor, &value) != 0) return -(int64_t)errno;
    return (int64_t)value.st_mode;
}
int64_t seen_macos_io_cache(int64_t descriptor, uint32_t uncached) {
    if (descriptor < 0 || descriptor > INT_MAX || uncached > 1) return -EINVAL;
    int result = fcntl((int)descriptor, F_NOCACHE, (int)uncached);
    return result < 0 ? -(int64_t)errno : (int64_t)result;
}
int64_t seen_macos_io_transfer(int64_t descriptor, void *buffer, uint64_t bytes, int64_t offset, uint32_t write) {
    if (descriptor < 0 || descriptor > INT_MAX || !buffer || bytes > SSIZE_MAX || offset < 0 || write > 1) return -EINVAL;
    ssize_t result = write ? pwrite((int)descriptor, buffer, (size_t)bytes, (off_t)offset) :
        pread((int)descriptor, buffer, (size_t)bytes, (off_t)offset);
    return result < 0 ? -(int64_t)errno : (int64_t)result;
}
#else
uint32_t seen_macos_io_abi(void) { return 0; }
int64_t seen_macos_io_open(const uint8_t*p,uint32_t f,uint32_t m) {(void)p;(void)f;(void)m;return -78;}
int64_t seen_macos_io_mode(int64_t d) {(void)d;return -78;}
int64_t seen_macos_io_cache(int64_t d,uint32_t u) {(void)d;(void)u;return -78;}
int64_t seen_macos_io_transfer(int64_t d,void*b,uint64_t n,int64_t o,uint32_t w) {(void)d;(void)b;(void)n;(void)o;(void)w;return -78;}
#endif
