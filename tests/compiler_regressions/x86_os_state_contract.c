#include "../../seen_runtime/seen_runtime.c"
#include <assert.h>

static void all_features(void) {
    memset(&g_cpu_features, 0, sizeof(g_cpu_features));
    g_cpu_features.detected = g_cpu_features.avx = g_cpu_features.avx2 = g_cpu_features.fma = 1;
    g_cpu_features.avx512f = g_cpu_features.avx512bw = g_cpu_features.avx512dq = 1;
    g_cpu_features.avx512vl = g_cpu_features.avx512vnni = g_cpu_features.avx512bf16 = 1;
}
int main(void) {
    const uint64_t incomplete[] = {0, 2, 4, 224};
    for (unsigned i = 0; i < sizeof(incomplete) / sizeof(incomplete[0]); ++i) {
        all_features(); seen_cpu_normalize_x86_state(incomplete[i]);
        assert(!g_cpu_features.avx && !g_cpu_features.avx2 && !g_cpu_features.fma);
        assert(!g_cpu_features.avx512f && !g_cpu_features.avx512vnni && !g_cpu_features.avx512bf16);
    }
    all_features(); seen_cpu_normalize_x86_state(6);
    assert(g_cpu_features.avx2 && g_cpu_features.fma && !g_cpu_features.avx512f);
    assert(!g_cpu_features.avx512vnni && !g_cpu_features.avx512bf16);
    all_features(); seen_cpu_normalize_x86_state(230);
    assert(g_cpu_features.avx512f && g_cpu_features.avx512vnni && g_cpu_features.avx512bf16);
    all_features(); g_cpu_features.avx = 0; seen_cpu_normalize_x86_state(230);
    assert(!g_cpu_features.avx2 && !g_cpu_features.avx512f);
    all_features(); g_cpu_features.avx512f = 0; seen_cpu_normalize_x86_state(230);
    assert(!g_cpu_features.avx512vnni && !g_cpu_features.avx512bf16);
    all_features(); g_cpu_features.fma = 0;
    assert(seen_cpu_simd_tier() != 3);
    puts("PASS: x86 instruction availability requires OS vector register state");
    return 0;
}
