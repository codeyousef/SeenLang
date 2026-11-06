#ifndef SEEN_MACOS_IO_ABI_H
#define SEEN_MACOS_IO_ABI_H
#include <stdint.h>
uint32_t seen_macos_io_abi(void);
int64_t seen_macos_io_open(const uint8_t *path, uint32_t flags, uint32_t mode);
int64_t seen_macos_io_mode(int64_t descriptor);
int64_t seen_macos_io_cache(int64_t descriptor, uint32_t uncached);
int64_t seen_macos_io_transfer(int64_t descriptor, void *buffer, uint64_t bytes, int64_t offset, uint32_t write);
#endif
