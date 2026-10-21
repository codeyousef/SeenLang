#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include "src/executor_socket_abi.h"
#include <assert.h>
#include <netinet/in.h>
#include <poll.h>
#include <stdint.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>
int main(void) {
    int server = socket(AF_INET, SOCK_DGRAM | SOCK_CLOEXEC, 0); assert(server >= 0);
    struct sockaddr_in address; memset(&address, 0, sizeof(address));
    address.sin_family = AF_INET; address.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    assert(bind(server, (struct sockaddr *)&address, sizeof(address)) == 0);
    socklen_t address_size = sizeof(address);
    assert(getsockname(server, (struct sockaddr *)&address, &address_size) == 0);
    const uint8_t local[] = {127, 0, 0, 1}; int64_t client = -1;
    assert(seen_async_datagram_open(local, 3, ntohs(address.sin_port), &client) == 1);
    assert(seen_async_datagram_open(local, 4, ntohs(address.sin_port), &client) == 0);
    uint16_t nonce = 0; assert(seen_async_datagram_nonce(&nonce) == 0);
    uint64_t count = 0;
    assert(seen_async_socket_write(client, "request", 7, &count) == 0 && count == 7);
    struct pollfd ready = {server, POLLIN, 0}; assert(poll(&ready, 1, 2000) == 1);
    char bytes[32]; struct sockaddr_storage peer; socklen_t peer_size = sizeof(peer);
    assert(recvfrom(server, bytes, sizeof(bytes), 0, (struct sockaddr *)&peer, &peer_size) == 7);
    assert(memcmp(bytes, "request", 7) == 0);
    assert(sendto(server, "reply", 5, 0, (struct sockaddr *)&peer, peer_size) == 5);
    ready.fd = (int)client; assert(poll(&ready, 1, 2000) == 1);
    assert(seen_async_datagram_receive(client, bytes, sizeof(bytes), &count) == 0 && count == 5);
    assert(memcmp(bytes, "reply", 5) == 0);
    assert(sendto(server, "truncated", 9, 0, (struct sockaddr *)&peer, peer_size) == 9);
    assert(poll(&ready, 1, 2000) == 1);
    assert(seen_async_datagram_receive(client, bytes, 2, &count) == 2 && count == 0);
    assert(seen_async_datagram_receive(client, bytes, sizeof(bytes), &count) == 8 && count == 0);
    assert(close((int)client) == 0); assert(close(server) == 0);
    return 0;
}
