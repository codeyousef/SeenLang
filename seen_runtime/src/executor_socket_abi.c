#if defined(__linux__) && !defined(_GNU_SOURCE)
#define _GNU_SOURCE
#endif
#include "executor_socket_abi.h"
#include <limits.h>
#include <stddef.h>
#include <string.h>
#if defined(__linux__) && defined(__x86_64__)
#include <errno.h>
#include <sys/socket.h>
#include <sys/random.h>
#include <netinet/in.h>
#include <unistd.h>

static int32_t seen_async_socket_errno(int error) {
    if (error == EAGAIN || error == EWOULDBLOCK || error == EINPROGRESS ||
        error == EALREADY) return 8;
    if (error == EINTR) return 5;
    if (error == EINVAL || error == EBADF || error == ENOTSOCK) return 1;
    if (error == ENOMEM || error == ENOBUFS || error == EMFILE || error == ENFILE)
        return 2;
    if (error == ENOSYS || error == EAFNOSUPPORT || error == EPROTONOSUPPORT)
        return 4;
    return 3;
}
static int seen_async_socket_fd(int64_t handle) {
    return handle >= 0 && handle <= INT_MAX;
}
static int32_t seen_async_socket_address(const uint8_t *address, uint64_t length,
    uint16_t port, struct sockaddr_storage *storage, socklen_t *size) {
    if (!address || !storage || !size) return 1;
    memset(storage, 0, sizeof(*storage));
    if (length == 4) {
        struct sockaddr_in *value = (struct sockaddr_in *)storage;
        value->sin_family = AF_INET; value->sin_port = htons(port);
        memcpy(&value->sin_addr, address, 4); *size = sizeof(*value); return 0;
    }
    if (length == 16) {
        struct sockaddr_in6 *value = (struct sockaddr_in6 *)storage;
        value->sin6_family = AF_INET6; value->sin6_port = htons(port);
        memcpy(&value->sin6_addr, address, 16); *size = sizeof(*value); return 0;
    }
    return 1;
}
int32_t seen_async_socket_open(int32_t family, int64_t *handle) {
    if (!handle || *handle != -1 || (family != 4 && family != 6)) return 1;
    int value = socket(family == 4 ? AF_INET : AF_INET6,
                       SOCK_STREAM | SOCK_NONBLOCK | SOCK_CLOEXEC, 0);
    if (value < 0) return seen_async_socket_errno(errno);
    *handle = value; return 0;
}
int32_t seen_async_socket_bind(int64_t handle, const uint8_t *address,
    uint64_t length, uint16_t port) {
    if (!seen_async_socket_fd(handle)) return 1;
    struct sockaddr_storage storage; socklen_t size = 0;
    int32_t status = seen_async_socket_address(address, length, port, &storage, &size);
    if (status != 0) return status;
    return bind((int)handle, (struct sockaddr *)&storage, size) == 0 ?
        0 : seen_async_socket_errno(errno);
}
int32_t seen_async_socket_listen(int64_t handle, int32_t backlog) {
    if (!seen_async_socket_fd(handle) || backlog <= 0) return 1;
    return listen((int)handle, backlog) == 0 ? 0 : seen_async_socket_errno(errno);
}
int32_t seen_async_socket_accept(int64_t handle, int64_t *client) {
    if (!seen_async_socket_fd(handle) || !client || *client != -1) return 1;
    int value = accept4((int)handle, NULL, NULL, SOCK_NONBLOCK | SOCK_CLOEXEC);
    if (value < 0) return seen_async_socket_errno(errno);
    *client = value; return 0;
}
int32_t seen_async_socket_connect(int64_t handle, const uint8_t *address,
    uint64_t length, uint16_t port) {
    if (!seen_async_socket_fd(handle)) return 1;
    struct sockaddr_storage storage; socklen_t size = 0;
    int32_t status = seen_async_socket_address(address, length, port, &storage, &size);
    if (status != 0) return status;
    return connect((int)handle, (struct sockaddr *)&storage, size) == 0 ?
        0 : seen_async_socket_errno(errno);
}
int32_t seen_async_socket_connect_status(int64_t handle) {
    if (!seen_async_socket_fd(handle)) return 1;
    int error = 0; socklen_t size = sizeof(error);
    if (getsockopt((int)handle, SOL_SOCKET, SO_ERROR, &error, &size) != 0)
        return seen_async_socket_errno(errno);
    return error == 0 ? 0 : seen_async_socket_errno(error);
}
int32_t seen_async_socket_read(int64_t handle, void *data, uint64_t length,
    uint64_t *completed) {
    if (!seen_async_socket_fd(handle) || !data || !completed || length == 0 ||
        length > (uint64_t)SSIZE_MAX) return 1;
    *completed = 0;
    ssize_t bytes = recv((int)handle, data, (size_t)length, MSG_DONTWAIT);
    if (bytes < 0) return seen_async_socket_errno(errno);
    *completed = (uint64_t)bytes; return 0;
}
int32_t seen_async_socket_write(int64_t handle, const void *data, uint64_t length,
    uint64_t *completed) {
    if (!seen_async_socket_fd(handle) || !data || !completed || length == 0 ||
        length > (uint64_t)SSIZE_MAX) return 1;
    *completed = 0;
    ssize_t bytes = send((int)handle, data, (size_t)length, MSG_DONTWAIT | MSG_NOSIGNAL);
    if (bytes < 0) return seen_async_socket_errno(errno);
    *completed = (uint64_t)bytes; return 0;
}
int32_t seen_async_socket_bound_port(int64_t handle, uint16_t *port) {
    if (!seen_async_socket_fd(handle) || !port) return 1;
    struct sockaddr_storage storage; socklen_t size = sizeof(storage);
    if (getsockname((int)handle, (struct sockaddr *)&storage, &size) != 0)
        return seen_async_socket_errno(errno);
    if (storage.ss_family == AF_INET) {
        *port = ntohs(((struct sockaddr_in *)&storage)->sin_port); return 0;
    }
    if (storage.ss_family == AF_INET6) {
        *port = ntohs(((struct sockaddr_in6 *)&storage)->sin6_port); return 0;
    }
    return 4;
}
int32_t seen_async_datagram_open(const uint8_t *address, uint64_t length,
    uint16_t port, int64_t *descriptor) {
    if (!descriptor || *descriptor != -1 || !port) return 1;
    struct sockaddr_storage storage; socklen_t size = 0;
    int32_t status = seen_async_socket_address(address, length, port, &storage, &size);
    if (status) return status;
    int fd = socket(length == 4 ? AF_INET : AF_INET6, SOCK_DGRAM | SOCK_NONBLOCK | SOCK_CLOEXEC, 0);
    if (fd < 0) return seen_async_socket_errno(errno);
    if (connect(fd, (struct sockaddr *)&storage, size) != 0) {
        int error = errno; close(fd); return seen_async_socket_errno(error);
    }
    *descriptor = fd; return 0;
}
int32_t seen_async_datagram_receive(int64_t descriptor, void *buffer,
    uint64_t capacity, uint64_t *completed) {
    if (!seen_async_socket_fd(descriptor) || !buffer || !completed ||
        !capacity || capacity > SSIZE_MAX) return 1;
    *completed = 0;
    ssize_t received = recv((int)descriptor, buffer, (size_t)capacity, MSG_DONTWAIT | MSG_TRUNC);
    if (received < 0) return seen_async_socket_errno(errno);
    if ((uint64_t)received > capacity) return 2;
    *completed = (uint64_t)received; return 0;
}
int32_t seen_async_datagram_nonce(uint16_t *value) {
    if (!value) return 1;
    ssize_t count = getrandom(value, sizeof(*value), GRND_NONBLOCK);
    if (count < 0) return seen_async_socket_errno(errno);
    return count == sizeof(*value) ? 0 : 3;
}

#else
int32_t seen_async_socket_open(int32_t f, int64_t *h) {(void)f;(void)h;return 4;}
int32_t seen_async_socket_bind(int64_t h,const uint8_t*a,uint64_t l,uint16_t p) {(void)h;(void)a;(void)l;(void)p;return 4;}
int32_t seen_async_socket_listen(int64_t h,int32_t b) {(void)h;(void)b;return 4;}
int32_t seen_async_socket_accept(int64_t h,int64_t*c) {(void)h;(void)c;return 4;}
int32_t seen_async_socket_connect(int64_t h,const uint8_t*a,uint64_t l,uint16_t p) {(void)h;(void)a;(void)l;(void)p;return 4;}
int32_t seen_async_socket_connect_status(int64_t h) {(void)h;return 4;}
int32_t seen_async_socket_read(int64_t h,void*d,uint64_t l,uint64_t*c) {(void)h;(void)d;(void)l;(void)c;return 4;}
int32_t seen_async_socket_write(int64_t h,const void*d,uint64_t l,uint64_t*c) {(void)h;(void)d;(void)l;(void)c;return 4;}
int32_t seen_async_socket_bound_port(int64_t h,uint16_t*p) {(void)h;(void)p;return 4;}
int32_t seen_async_datagram_open(const uint8_t*a,uint64_t n,uint16_t p,int64_t*d) {(void)a;(void)n;(void)p;(void)d;return 4;}
int32_t seen_async_datagram_receive(int64_t d,void*b,uint64_t n,uint64_t*c) {(void)d;(void)b;(void)n;(void)c;return 4;}
int32_t seen_async_datagram_nonce(uint16_t*v) {(void)v;return 4;}
#endif
