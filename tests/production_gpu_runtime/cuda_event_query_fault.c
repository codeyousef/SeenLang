#define _GNU_SOURCE
#include <cuda_runtime_api.h>
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static unsigned queries;
cudaError_t CUDARTAPI cudaEventQuery(cudaEvent_t event) {
    ++queries;
    const char *mode = getenv("SEEN_CUDA_QUERY_FAULT");
    if (queries == 1 && mode && strcmp(mode, "pending") == 0) return cudaErrorNotReady;
    if (queries == 1 && mode && strcmp(mode, "error") == 0) return cudaErrorUnknown;
    typedef cudaError_t (*Query)(cudaEvent_t);
    Query next = (Query)dlsym(RTLD_NEXT, "cudaEventQuery");
    if (!next) abort();
    return next(event);
}
cudaError_t CUDARTAPI cudaEventSynchronize(cudaEvent_t event) {
    (void)event; fputs("FAIL: event query path blocked on an event\n", stderr); abort();
}
cudaError_t CUDARTAPI cudaStreamSynchronize(cudaStream_t stream) {
    (void)stream; fputs("FAIL: event query path blocked on a stream\n", stderr); abort();
}
cudaError_t CUDARTAPI cudaDeviceSynchronize(void) {
    fputs("FAIL: event query path blocked on a device\n", stderr); abort();
}
__attribute__((destructor)) static void trace_query(void) {
    // The bounded timeout launcher also inherits LD_PRELOAD but makes no SDK
    // calls. Only the application emits a trace; the harness requires one.
    if (!queries) return;
    fprintf(stderr, "CUDA_QUERY_TRACE queries=%u waits=0\n", queries);
}
