// Matrix Multiplication Benchmark (SGEMM)
// High-performance matrix multiplication with cache blocking
// Matches Seen implementation: 1920x1920 double, block_size=48, 3 warmup + 5 measured iterations

#include <stdio.h>
#include <stdlib.h>
#include <time.h>

#define SIZE 1920
#define BLOCK_SIZE 48
#define WARMUP_RUNS 3
#define ITERATIONS 5

static inline int min_int(int a, int b) {
    return a < b ? a : b;
}

static void matrix_fill_random(double *data, int total, int seed) {
    int current_seed = seed;
    for (int i = 0; i < total; i++) {
        current_seed = (current_seed * 1103515245 + 12345) % 2147483647;
        if (current_seed < 0) {
            current_seed = -current_seed;
        }
        int value_int = current_seed % 10000;
        data[i] = (double)value_int / 10000.0;
    }
}

static void matrix_multiply(const double *a, const double *b, double *c, int size) {
    for (int ii = 0; ii < size; ii += BLOCK_SIZE) {
        for (int jj = 0; jj < size; jj += BLOCK_SIZE) {
            for (int kk = 0; kk < size; kk += BLOCK_SIZE) {
                int i_end = min_int(ii + BLOCK_SIZE, size);
                int j_end = min_int(jj + BLOCK_SIZE, size);
                int k_end = min_int(kk + BLOCK_SIZE, size);

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

static double compute_checksum(const double *data, int total) {
    double sum = 0.0;
    for (int i = 0; i < total; i++) {
        sum += data[i];
    }
    return sum;
}

static double get_time_seconds(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec + (double)ts.tv_nsec / 1e9;
}

int main(void) {
    int size = SIZE;
    int total = size * size;

    printf("Matrix Multiplication Benchmark (SGEMM)\n");
    printf("Matrix size: %dx%d\n", size, size);

    double *a = (double *)calloc(total, sizeof(double));
    double *b = (double *)calloc(total, sizeof(double));
    double *c = (double *)calloc(total, sizeof(double));

    if (!a || !b || !c) {
        fprintf(stderr, "Allocation failed\n");
        return 1;
    }

    matrix_fill_random(a, total, 12345);
    matrix_fill_random(b, total, 67890);

    printf("Warming up (%d runs)...\n", WARMUP_RUNS);
    for (int w = 0; w < WARMUP_RUNS; w++) {
        matrix_multiply(a, b, c, size);
    }

    printf("Running measured iterations...\n");
    double min_time = 1e9;

    for (int i = 0; i < ITERATIONS; i++) {
        double *c_fresh = (double *)calloc(total, sizeof(double));
        if (!c_fresh) {
            fprintf(stderr, "Allocation failed\n");
            return 1;
        }

        double start = get_time_seconds();
        matrix_multiply(a, b, c_fresh, size);
        double end = get_time_seconds();

        double elapsed = (end - start) * 1000.0;
        if (elapsed < min_time) {
            min_time = elapsed;
        }
        free(c_fresh);
    }

    double checksum = compute_checksum(c, total);
    double size_f = (double)size;
    double gflops = (2.0 * size_f * size_f * size_f) / (min_time / 1000.0) / 1e9;

    printf("Checksum: %f\n", checksum);
    printf("Min time: %f ms\n", min_time);
    printf("Performance: %f GFLOPS\n", gflops);

    free(a);
    free(b);
    free(c);

    return 0;
}
