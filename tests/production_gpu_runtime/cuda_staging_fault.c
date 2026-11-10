#define _GNU_SOURCE
#include <cuda_runtime_api.h>
#include <dlfcn.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <sys/syscall.h>
#include <sys/resource.h>
#include <unistd.h>

#define NEXT(name) __typeof__(&name) next = (__typeof__(&name))dlsym(RTLD_NEXT, #name); if (!next) abort()
static void *live[4][256];
static uint64_t live_bytes[4][256], acquired[4], released[4];
static unsigned live_counts[4], peak_objects;
static uint64_t staging_acquired, staging_released, staging_live, staging_peak;
static unsigned hosts, devices, streams, events, copies, records;
static uint64_t event_queries, stream_queries;
static unsigned failed, pending_once;
static _Atomic int late_clock;
static long warm_rss_kb;
static int64_t soak_started_ns;
#if __has_feature(address_sanitizer)
/* Default ASan quarantine and shadow grow even with exact live cleanup.
 * Do not shrink/disable them to manufacture a flat RSS result. The enclosing
 * aggregate physical-memory limit remains unchanged for every profile. */
static const long soak_rss_growth_kb = 327680;
#else
static const long soak_rss_growth_kb = 16384;
#endif
static int mode(const char *name) {
    const char *value = getenv("SEEN_CUDA_STAGING_FAULT");
    return value && strcmp(value, name) == 0;
}
static int soaking(void) { return mode("soak") || mode("lifecycle-soak"); }
static int once(const char *name) {
    if (!mode(name) || failed) return 0;
    failed = 1; return 1;
}
static void track(unsigned kind, void *value, int adding, uint64_t bytes) {
    if (!value) abort();
    for (unsigned i = 0; i < 256; ++i) if (live[kind][i] == value) {
        if (adding) abort();
        if (!live_counts[kind]) abort();
        --live_counts[kind]; ++released[kind];
        if (kind == 0) { staging_live -= live_bytes[kind][i]; staging_released += live_bytes[kind][i]; }
        live[kind][i] = NULL; live_bytes[kind][i] = 0; return;
    }
    if (!adding) abort();
    for (unsigned i = 0; i < 256; ++i) if (!live[kind][i]) {
        live[kind][i] = value; live_bytes[kind][i] = bytes;
        ++live_counts[kind]; ++acquired[kind];
        unsigned total = live_counts[0] + live_counts[1] + live_counts[2] + live_counts[3];
        if (total > peak_objects) peak_objects = total;
        if (kind == 0) { staging_live += bytes; staging_acquired += bytes; if (staging_live > staging_peak) staging_peak = staging_live; }
        return;
    }
    abort();
}
cudaError_t CUDARTAPI cudaHostAlloc(void **pointer, size_t bytes, unsigned flags) {
    ++hosts;
    if (mode("host-second") && hosts == 2) { *pointer = NULL; return cudaErrorMemoryAllocation; }
    NEXT(cudaHostAlloc); cudaError_t status = next(pointer, bytes, flags);
    if (status == cudaSuccess) track(0, *pointer, 1, bytes);
    return status;
}
cudaError_t CUDARTAPI cudaMalloc(void **pointer, size_t bytes) {
    ++devices; NEXT(cudaMalloc); cudaError_t status = next(pointer, bytes);
    if (status == cudaSuccess) track(1, *pointer, 1, bytes);
    return status;
}
cudaError_t CUDARTAPI cudaStreamCreateWithFlags(cudaStream_t *stream, unsigned flags) {
    ++streams; NEXT(cudaStreamCreateWithFlags); cudaError_t status = next(stream, flags);
    if (status == cudaSuccess) track(2, (void *)*stream, 1, 0);
    return status;
}
cudaError_t CUDARTAPI cudaEventCreate(cudaEvent_t *event) {
    ++events;
    if (mode("event-second") && events == 2) { *event = NULL; return cudaErrorMemoryAllocation; }
    NEXT(cudaEventCreate); cudaError_t status = next(event);
    if (status == cudaSuccess) track(3, (void *)*event, 1, 0);
    return status;
}
cudaError_t CUDARTAPI cudaFreeHost(void *pointer) {
    if (once("close-host")) return cudaErrorUnknown;
    NEXT(cudaFreeHost); cudaError_t status = next(pointer);
    if (status == cudaSuccess) track(0, pointer, 0, 0);
    return status;
}
cudaError_t CUDARTAPI cudaFree(void *pointer) {
    NEXT(cudaFree); cudaError_t status = next(pointer);
    if (status == cudaSuccess && pointer) track(1, pointer, 0, 0);
    return status;
}
cudaError_t CUDARTAPI cudaStreamDestroy(cudaStream_t stream) {
    if (once("close-stream")) return cudaErrorUnknown;
    NEXT(cudaStreamDestroy); cudaError_t status = next(stream);
    if (status == cudaSuccess) track(2, (void *)stream, 0, 0);
    return status;
}
cudaError_t CUDARTAPI cudaEventDestroy(cudaEvent_t event) {
    NEXT(cudaEventDestroy); cudaError_t status = next(event);
    if (status == cudaSuccess) track(3, (void *)event, 0, 0);
    return status;
}
cudaError_t CUDARTAPI cudaMemcpyAsync(void *to, const void *from, size_t bytes, enum cudaMemcpyKind kind, cudaStream_t stream) {
    ++copies;
    if (soaking() && copies == 1) {
        struct timespec sample;
        if (syscall(SYS_clock_gettime, CLOCK_MONOTONIC, &sample)) abort();
        soak_started_ns = (int64_t)sample.tv_sec * INT64_C(1000000000) + sample.tv_nsec;
    }
    NEXT(cudaMemcpyAsync); cudaError_t status = next(to, from, bytes, kind, stream);
    if (status == cudaSuccess && (once("copy-error") || (mode("copy-second-error") && copies == 2))) return cudaErrorUnknown;
    return status;
}
cudaError_t CUDARTAPI cudaEventRecord(cudaEvent_t event, cudaStream_t stream) {
    ++records; NEXT(cudaEventRecord); cudaError_t status = next(event, stream);
    if (status == cudaSuccess && mode("late-timeout")) late_clock = 1;
    if (status == cudaSuccess && soaking() && records == 768) {
        struct rusage sample;
        if (getrusage(RUSAGE_SELF, &sample)) abort();
        warm_rss_kb = sample.ru_maxrss;
    }
    if (status == cudaSuccess && once("record-error")) return cudaErrorUnknown;
    return status;
}
cudaError_t CUDARTAPI cudaEventQuery(cudaEvent_t event) {
    ++event_queries;
    if (mode("query-permanent")) return cudaErrorUnknown;
    if (once("query-error")) return cudaErrorUnknown;
    NEXT(cudaEventQuery); return next(event);
}
cudaError_t CUDARTAPI cudaStreamQuery(cudaStream_t stream) {
    ++stream_queries;
    if ((mode("record-error") || mode("query-permanent")) && !pending_once) { pending_once = 1; return cudaErrorNotReady; }
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
    if (mode("query-permanent") && (event_queries != 1 || stream_queries < 2)) {
        fputs("FAIL: unusable event did not switch to private-stream retirement\n", stderr); _Exit(98);
    }
    if (soaking()) {
        struct rusage sample;
        if (getrusage(RUSAGE_SELF, &sample) || warm_rss_kb <= 0 ||
            sample.ru_maxrss > warm_rss_kb + soak_rss_growth_kb) {
            fputs("FAIL: bounded CUDA soak RSS envelope exceeded\n", stderr); _Exit(99);
        }
        fprintf(stderr, "CUDA_STAGING_RSS warm_peak_kb=%ld final_peak_kb=%ld growth_limit_kb=%ld\n",
                warm_rss_kb, sample.ru_maxrss, soak_rss_growth_kb);
        struct timespec finished;
        if (syscall(SYS_clock_gettime, CLOCK_MONOTONIC, &finished)) abort();
        int64_t elapsed = (int64_t)finished.tv_sec * INT64_C(1000000000) + finished.tv_nsec - soak_started_ns;
        if (soak_started_ns <= 0 || elapsed < 0) abort();
        fprintf(stderr, "CUDA_STAGING_SOAK_ELAPSED_MS=%lld\n", (long long)(elapsed / INT64_C(1000000)));
        if (mode("lifecycle-soak")) {
            if (copies < 120002 || (copies - 2) % 12 || records != copies || elapsed < INT64_C(60000000000)) abort();
            unsigned cycles = (copies - 2) / 12;
            uint64_t total_acquired = 0, total_released = 0;
            for (unsigned k = 0; k < 4; ++k) { total_acquired += acquired[k]; total_released += released[k]; }
            if (cycles > 1000000 || hosts != (cycles + 1) * 3 || devices != hosts || events != hosts || streams != cycles + 1 ||
                total_acquired != (uint64_t)(cycles + 1) * 10 || total_released != total_acquired ||
                staging_live || staging_acquired != staging_released || staging_peak != 384 || peak_objects != 20) abort();
            fprintf(stderr, "CUDA_STAGING_LIFECYCLE_EVIDENCE {\"schema\":\"seen-cuda-staging-lifecycle-v1\",\"cycles\":%u,\"copies\":%u,\"reuses_per_child\":4,\"elapsed_ms\":%lld,\"gpu_objects\":{\"baseline\":0,\"peak\":%u,\"final\":0,\"acquired\":%llu,\"released\":%llu},\"requested_pinned_bytes\":{\"baseline\":0,\"peak\":%llu,\"final\":0,\"acquired\":%llu,\"released\":%llu},\"physical_vram_measured\":false}\n",
                    cycles, copies, (long long)(elapsed / INT64_C(1000000)), peak_objects,
                    (unsigned long long)total_acquired, (unsigned long long)total_released,
                    (unsigned long long)staging_peak, (unsigned long long)staging_acquired, (unsigned long long)staging_released);
        }
    }
    fprintf(stderr, "CUDA_STAGING_TRACE host=%u device=%u stream=%u event=%u copies=%u record=%u event_query=%llu stream_query=%llu drained=1 waits=0\n",
        hosts, devices, streams, events, copies, records,
        (unsigned long long)event_queries, (unsigned long long)stream_queries);
}
