// Matrix Multiplication Benchmark (SGEMM)
// Same algorithm as Seen: cache-blocked matrix multiply
// size=1024, block_size=64, seeds 12345/67890
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <time.h>

static double* matrix_new(int size) {
    return (double*)calloc((size_t)size * size, sizeof(double));
}

static void matrix_fill_random(double* data, int total, int64_t seed) {
    int64_t current_seed = seed;
    for (int i = 0; i < total; i++) {
        current_seed = (current_seed * 1103515245 + 12345) % 2147483647;
        if (current_seed < 0) current_seed = -current_seed;
        int64_t value_int = current_seed % 10000;
        data[i] = (double)value_int / 10000.0;
    }
}

static inline int min_int(int a, int b) { return a < b ? a : b; }

static void matrix_multiply(const double* a, const double* b, double* c, int size) {
    int block_size = 64;
    for (int ii = 0; ii < size; ii += block_size) {
        for (int jj = 0; jj < size; jj += block_size) {
            for (int kk = 0; kk < size; kk += block_size) {
                int i_end = min_int(ii + block_size, size);
                int j_end = min_int(jj + block_size, size);
                int k_end = min_int(kk + block_size, size);
                for (int i = ii; i < i_end; i++) {
                    for (int j = jj; j < j_end; j++) {
                        double sum = c[i * size + j];
                        for (int k = kk; k < k_end; k++) {
                            sum += a[i * size + k] * b[k * size + j];
                        }
                        c[i * size + j] = sum;
                    }
                }
            }
        }
    }
}

static double compute_checksum(const double* data, int total) {
    double sum = 0.0;
    for (int i = 0; i < total; i++) sum += data[i];
    return sum;
}

static double get_time_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000.0 + ts.tv_nsec / 1000000.0;
}

int main(void) {
    int size = 1024;
    int total = size * size;

    printf("Matrix Multiplication Benchmark (SGEMM)\n");
    printf("Matrix size: %dx%d\n", size, size);

    double* a = matrix_new(size);
    double* b = matrix_new(size);
    double* c = matrix_new(size);

    matrix_fill_random(a, total, 12345);
    matrix_fill_random(b, total, 67890);

    int warmup_runs = 3;
    printf("Warming up (%d runs)...\n", warmup_runs);
    for (int w = 0; w < warmup_runs; w++) {
        matrix_multiply(a, b, c, size);
    }

    printf("Running measured iterations...\n");
    int iterations = 5;
    double min_time = 1e18;

    for (int i = 0; i < iterations; i++) {
        double* c_fresh = matrix_new(size);
        double start = get_time_ms();
        matrix_multiply(a, b, c_fresh, size);
        double elapsed = get_time_ms() - start;
        if (elapsed < min_time) min_time = elapsed;
        free(c_fresh);
    }

    double checksum = compute_checksum(c, total);
    double size_f = (double)size;
    double gflops = (2.0 * size_f * size_f * size_f) / (min_time / 1000.0) / 1e9;

    printf("Checksum: %.6f\n", checksum);
    printf("Min time: %.6f ms\n", min_time);
    printf("Performance: %.6f GFLOPS\n", gflops);

    free(a); free(b); free(c);
    return 0;
}
