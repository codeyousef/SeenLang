#define _GNU_SOURCE
#include <cuda_runtime_api.h>
#include <dlfcn.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <sys/syscall.h>
#include <unistd.h>

#define NEXT(name) __typeof__(&name) next = (__typeof__(&name))dlsym(RTLD_NEXT, #name); if (!next) abort()
static void *live[4][256];
static unsigned hosts, devices, streams, events, copies, records, event_queries, stream_queries;
static unsigned failed, pending_once;
static int late_clock;
static int mode(const char *name) {
    const char *value = getenv("SEEN_CUDA_STAGING_FAULT");
    return value && strcmp(value, name) == 0;
}
static int once(const char *name) {
    if (!mode(name) || failed) return 0;
    failed = 1; return 1;
}
static void track(unsigned kind, void *value, int adding) {
    if (!value) abort();
    for (unsigned i = 0; i < 256; ++i) if (live[kind][i] == value) {
        if (adding) abort();
        live[kind][i] = NULL; return;
    }
    if (!adding) abort();
    for (unsigned i = 0; i < 256; ++i) if (!live[kind][i]) { live[kind][i] = value; return; }
    abort();
}
cudaError_t CUDARTAPI cudaHostAlloc(void **pointer, size_t bytes, unsigned flags) {
    ++hosts;
    if (mode("host-second") && hosts == 2) { *pointer = NULL; return cudaErrorMemoryAllocation; }
    NEXT(cudaHostAlloc); cudaError_t status = next(pointer, bytes, flags);
    if (status == cudaSuccess) track(0, *pointer, 1);
    return status;
}
cudaError_t CUDARTAPI cudaMalloc(void **pointer, size_t bytes) {
    ++devices; NEXT(cudaMalloc); cudaError_t status = next(pointer, bytes);
    if (status == cudaSuccess) track(1, *pointer, 1);
    return status;
}
cudaError_t CUDARTAPI cudaStreamCreateWithFlags(cudaStream_t *stream, unsigned flags) {
    ++streams; NEXT(cudaStreamCreateWithFlags); cudaError_t status = next(stream, flags);
    if (status == cudaSuccess) track(2, (void *)*stream, 1);
    return status;
}
cudaError_t CUDARTAPI cudaEventCreate(cudaEvent_t *event) {
    ++events;
    if (mode("event-second") && events == 2) { *event = NULL; return cudaErrorMemoryAllocation; }
    NEXT(cudaEventCreate); cudaError_t status = next(event);
    if (status == cudaSuccess) track(3, (void *)*event, 1);
    return status;
}
cudaError_t CUDARTAPI cudaFreeHost(void *pointer) {
    if (once("close-host")) return cudaErrorUnknown;
    NEXT(cudaFreeHost); cudaError_t status = next(pointer);
    if (status == cudaSuccess) track(0, pointer, 0);
    return status;
}
cudaError_t CUDARTAPI cudaFree(void *pointer) {
    NEXT(cudaFree); cudaError_t status = next(pointer);
    if (status == cudaSuccess && pointer) track(1, pointer, 0);
    return status;
}
cudaError_t CUDARTAPI cudaStreamDestroy(cudaStream_t stream) {
    if (once("close-stream")) return cudaErrorUnknown;
    NEXT(cudaStreamDestroy); cudaError_t status = next(stream);
    if (status == cudaSuccess) track(2, (void *)stream, 0);
    return status;
}
cudaError_t CUDARTAPI cudaEventDestroy(cudaEvent_t event) {
    NEXT(cudaEventDestroy); cudaError_t status = next(event);
    if (status == cudaSuccess) track(3, (void *)event, 0);
    return status;
}
cudaError_t CUDARTAPI cudaMemcpyAsync(void *to, const void *from, size_t bytes, enum cudaMemcpyKind kind, cudaStream_t stream) {
    ++copies; NEXT(cudaMemcpyAsync); cudaError_t status = next(to, from, bytes, kind, stream);
    if (status == cudaSuccess && (once("copy-error") || (mode("copy-second-error") && copies == 2))) return cudaErrorUnknown;
    return status;
}
cudaError_t CUDARTAPI cudaEventRecord(cudaEvent_t event, cudaStream_t stream) {
    ++records; NEXT(cudaEventRecord); cudaError_t status = next(event, stream);
    if (status == cudaSuccess && mode("late-timeout")) late_clock = 1;
    if (status == cudaSuccess && once("record-error")) return cudaErrorUnknown;
    return status;
}
cudaError_t CUDARTAPI cudaEventQuery(cudaEvent_t event) {
    ++event_queries;
    if (once("query-error")) return cudaErrorUnknown;
    NEXT(cudaEventQuery); return next(event);
}
cudaError_t CUDARTAPI cudaStreamQuery(cudaStream_t stream) {
    ++stream_queries;
    if (mode("record-error") && !pending_once) { pending_once = 1; return cudaErrorNotReady; }
    NEXT(cudaStreamQuery); return next(stream);
}
cudaError_t CUDARTAPI cudaEventSynchronize(cudaEvent_t event) {
    (void)event; fputs("FAIL: staging event synchronization\n", stderr); abort();
}
cudaError_t CUDARTAPI cudaStreamSynchronize(cudaStream_t stream) {
    (void)stream; fputs("FAIL: staging stream synchronization\n", stderr); abort();
}
cudaError_t CUDARTAPI cudaDeviceSynchronize(void) {
    fputs("FAIL: staging device synchronization\n", stderr); abort();
}
int clock_gettime(clockid_t clock, struct timespec *value) {
    int status = (int)syscall(SYS_clock_gettime, clock, value);
    if (!status && clock == CLOCK_MONOTONIC && late_clock) value->tv_sec += 2;
    return status;
}
__attribute__((destructor)) static void require_drain(void) {
    for (unsigned k = 0; k < 4; ++k) for (unsigned i = 0; i < 256; ++i) if (live[k][i]) {
        fputs("FAIL: CUDA staging retained native ownership\n", stderr); _Exit(97);
    }
    if (!hosts && !streams) return; // timeout launcher also inherits preload
    if (mode("record-error") && (event_queries || stream_queries < 2)) {
        fputs("FAIL: unrecorded event used as completion proof\n", stderr); _Exit(98);
    }
    fprintf(stderr, "CUDA_STAGING_TRACE host=%u device=%u stream=%u event=%u copies=%u record=%u event_query=%u stream_query=%u drained=1 waits=0\n",
        hosts, devices, streams, events, copies, records, event_queries, stream_queries);
}
