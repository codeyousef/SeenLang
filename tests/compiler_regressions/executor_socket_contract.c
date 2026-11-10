#if !defined(_GNU_SOURCE)
#define _GNU_SOURCE
#endif
#include "src/executor_socket_abi.h"
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <stdint.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

static void wait_ready(int64_t fd, short events) {
    struct pollfd item = {(int)fd, events, 0};
    int result;
    do { result = poll(&item, 1, 2000); } while (result < 0 && errno == EINTR);
    assert(result == 1);
}
int main(void) {
    int64_t listener = -1, client = -1, accepted = -1;
    const uint8_t local[] = {127, 0, 0, 1};
    assert(seen_async_socket_open(0, &listener) == 1 && listener == -1);
    assert(seen_async_socket_open(4, &listener) == 0);
    assert(fcntl((int)listener, F_GETFL) & O_NONBLOCK);
    assert(fcntl((int)listener, F_GETFD) & FD_CLOEXEC);
    assert(seen_async_socket_bind(listener, local, 3, 0) == 1);
    assert(seen_async_socket_bind(listener, local, sizeof(local), 0) == 0);
    assert(seen_async_socket_listen(listener, 2) == 0);
    uint16_t port = 0;
    assert(seen_async_socket_bound_port(listener, &port) == 0 && port != 0);
    assert(seen_async_socket_accept(listener, &accepted) == 8 && accepted == -1);
    assert(seen_async_socket_open(4, &client) == 0);
    int32_t status = seen_async_socket_connect(client, local, sizeof(local), port);
    assert(status == 0 || status == 8);
    wait_ready(client, POLLOUT);
    assert(seen_async_socket_connect_status(client) == 0);
    wait_ready(listener, POLLIN);
    assert(seen_async_socket_accept(listener, &accepted) == 0);
    assert(fcntl((int)accepted, F_GETFL) & O_NONBLOCK);
    assert(fcntl((int)accepted, F_GETFD) & FD_CLOEXEC);
    char buffer[8] = {0}; uint64_t completed = 99;
    assert(seen_async_socket_read(accepted, buffer, sizeof(buffer), &completed) == 8);
    assert(completed == 0);
    assert(seen_async_socket_write(client, "abc", 3, &completed) == 0 && completed == 3);
    wait_ready(accepted, POLLIN);
    assert(seen_async_socket_read(accepted, buffer, sizeof(buffer), &completed) == 0);
    assert(completed == 3 && memcmp(buffer, "abc", 3) == 0);
    assert(shutdown((int)client, SHUT_WR) == 0);
    wait_ready(accepted, POLLIN);
    assert(seen_async_socket_read(accepted, buffer, sizeof(buffer), &completed) == 0 && completed == 0);
    assert(seen_async_socket_read(-1, buffer, sizeof(buffer), &completed) == 1);
    assert(seen_async_socket_write(client, buffer, UINT64_MAX, &completed) == 1);
    assert(close((int)accepted) == 0); assert(close((int)client) == 0);
    assert(close((int)listener) == 0);
    return 0;
}
