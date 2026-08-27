#include <immintrin.h>

__attribute__((noinline)) static void exercise(float *values) {
#if defined(SEEN_PROBE_AVX512)
    __m512 lanes = _mm512_set1_ps(1.0f);
    _mm512_storeu_ps(values, lanes);
#elif defined(SEEN_PROBE_AVX)
    __m256 lanes = _mm256_set1_ps(1.0f);
    _mm256_storeu_ps(values, lanes);
#else
    values[0] = 1.0f;
#endif
}

int main(void) {
    float values[16] = {0};
    exercise(values);
    return values[0] == 1.0f ? 0 : 1;
}
