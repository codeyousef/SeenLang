#include "seen_cuda.h"
#include <cstdio>
#include <cstring>

#define REQUIRE(x) do { if (!(x)) { std::fprintf(stderr, "FAIL:%d: %s\n", __LINE__, #x); return 1; } } while (0)
#define OK(x) REQUIRE((x).code == SEEN_CUDA_OK)

struct Owners {
    SeenCudaHandle host = 0, device = 0, stream = 0, event = 0;
    ~Owners() {
        // A failed test still releases acquired owners. Passing paths close
        // explicitly only after query observed completion, never by waiting.
        seen_cuda_event_destroy(&event);
        seen_cuda_stream_destroy(&stream);
        seen_cuda_free(&device);
        seen_cuda_host_free(&host);
    }
};

int main(int argc, char **argv) {
    REQUIRE(argc == 2);
    const bool pending = std::strcmp(argv[1], "pending") == 0;
    const bool error = std::strcmp(argv[1], "error") == 0;
    REQUIRE(pending || error || std::strcmp(argv[1], "reference") == 0);
    int32_t ready = 77;
    REQUIRE(seen_cuda_event_query(0, &ready).code == SEEN_CUDA_INVALID_ARGUMENT);
    REQUIRE(ready == 0);
    REQUIRE(seen_cuda_event_query(0, nullptr).code == SEEN_CUDA_INVALID_ARGUMENT);
    SeenCudaDeviceInfo info{};
    OK(seen_cuda_device_get(0, &info));
    REQUIRE(std::strstr(info.name, "RTX 4090") != nullptr);
    REQUIRE(info.compute_major == 8 && info.compute_minor == 9);
    std::printf("device=%s uuid=%s\n", info.name, info.uuid);
    Owners owner;
    OK(seen_cuda_host_alloc(66, &owner.host));
    OK(seen_cuda_malloc(0, 33, &owner.device));
    OK(seen_cuda_stream_create(0, &owner.stream));
    OK(seen_cuda_event_create(0, &owner.event));
    void *host_pointer = nullptr, *device_pointer = nullptr;
    uint64_t bytes = 0; int32_t device = -1;
    OK(seen_cuda_host_allocation_address(owner.host, &host_pointer, &bytes));
    REQUIRE(bytes == 66);
    OK(seen_cuda_allocation_address(owner.device, &device_pointer, &bytes, &device));
    REQUIRE(bytes == 33 && device == 0);
    auto *host = static_cast<unsigned char *>(host_pointer);
    unsigned queries = 0;
    for (unsigned cycle = 0; cycle < 64; ++cycle) {
        for (unsigned i = 0; i < 33; ++i) { host[i] = static_cast<unsigned char>(255 - i - cycle); host[33 + i] = 0; }
        OK(seen_cuda_memcpy_async(device_pointer, host, 33, SEEN_CUDA_COPY_HOST_TO_DEVICE, owner.stream));
        OK(seen_cuda_memcpy_async(host + 33, device_pointer, 33, SEEN_CUDA_COPY_DEVICE_TO_HOST, owner.stream));
        OK(seen_cuda_event_record(owner.event, owner.stream));
        ready = 77;
        SeenCudaStatus observed = seen_cuda_event_query(owner.event, &ready); ++queries;
        if (cycle == 0 && error) {
            REQUIRE(observed.code == SEEN_CUDA_RUNTIME_ERROR && observed.native_code != 0 && ready == 0);
        } else {
            REQUIRE(observed.code == SEEN_CUDA_OK && (ready == 0 || ready == 1));
            if (cycle == 0 && pending) REQUIRE(ready == 0);
        }
        unsigned fuel = 0;
        while (!ready && fuel < 1000000) {
            ready = 77; OK(seen_cuda_event_query(owner.event, &ready));
            REQUIRE(ready == 0 || ready == 1); ++fuel; ++queries;
        }
        REQUIRE(ready == 1);
        for (unsigned i = 0; i < 33; ++i) REQUIRE(host[33 + i] == host[i]);
    }
    OK(seen_cuda_event_destroy(&owner.event));
    ready = 77;
    REQUIRE(seen_cuda_event_query(owner.event, &ready).code == SEEN_CUDA_INVALID_ARGUMENT && ready == 0);
    OK(seen_cuda_event_destroy(&owner.event));
    OK(seen_cuda_stream_destroy(&owner.stream));
    OK(seen_cuda_free(&owner.device)); OK(seen_cuda_host_free(&owner.host));
    std::printf("PASS: CUDA nonblocking query pinned byte parity reused event 64 cycles queries=%u\n", queries);
    return 0;
}
