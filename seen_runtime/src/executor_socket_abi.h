#ifndef SEEN_EXECUTOR_SOCKET_ABI_H
#define SEEN_EXECUTOR_SOCKET_ABI_H
#include <stdint.h>
/* 0=success, 1=invalid, 2=limit, 3=unavailable, 4=unsupported,
 * 5=interrupted, 8=would-block/in-progress. No retry or protocol policy. */
int32_t seen_async_socket_open(int32_t family, int64_t *handle);
int32_t seen_async_socket_bind(int64_t handle, const uint8_t *address,
    uint64_t length, uint16_t port);
int32_t seen_async_socket_listen(int64_t handle, int32_t backlog);
int32_t seen_async_socket_accept(int64_t handle, int64_t *client);
int32_t seen_async_socket_connect(int64_t handle, const uint8_t *address,
    uint64_t length, uint16_t port);
int32_t seen_async_socket_connect_status(int64_t handle);
int32_t seen_async_socket_read(int64_t handle, void *data, uint64_t length,
    uint64_t *completed);
int32_t seen_async_socket_write(int64_t handle, const void *data, uint64_t length,
    uint64_t *completed);
int32_t seen_async_socket_bound_port(int64_t handle, uint16_t *port);
#endif
