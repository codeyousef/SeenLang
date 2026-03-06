// Spectral Norm Benchmark
// Classic CLBG benchmark: eigenvalue approximation of infinite matrix
// Matches Seen implementation: n=5500, 10 power iterations, 3 warmup + 5 measured

#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include <time.h>

#define N_SIZE 5500
#define WARMUP_RUNS 3
#define WARMUP_N 500
#define ITERATIONS 5
#define POWER_ITERS 10

static inline double eval_a(int i, int j) {
    int ij = i + j;
    int denom = ij * (ij + 1) / 2 + i + 1;
    return 1.0 / (double)denom;
}

static void multiply_av(const double *v, double *out, int n) {
    for (int i = 0; i < n; i++) {
        double sum = 0.0;
        for (int j = 0; j < n; j++) {
            sum += eval_a(i, j) * v[j];
        }
        out[i] = sum;
    }
}

static void multiply_atv(const double *v, double *out, int n) {
    for (int i = 0; i < n; i++) {
        double sum = 0.0;
        for (int j = 0; j < n; j++) {
            sum += eval_a(j, i) * v[j];
        }
        out[i] = sum;
    }
}

static void multiply_atav(const double *v, double *out, double *tmp, int n) {
    multiply_av(v, tmp, n);
    multiply_atv(tmp, out, n);
}

static double run_spectral_norm(int n) {
    double *u = (double *)malloc(n * sizeof(double));
    double *v = (double *)calloc(n, sizeof(double));
    double *tmp = (double *)calloc(n, sizeof(double));

    for (int i = 0; i < n; i++) {
        u[i] = 1.0;
    }

    for (int iter = 0; iter < POWER_ITERS; iter++) {
        multiply_atav(u, v, tmp, n);
        multiply_atav(v, u, tmp, n);
    }

    double vbv = 0.0;
    double vv = 0.0;
    for (int i = 0; i < n; i++) {
        vbv += u[i] * v[i];
        vv += v[i] * v[i];
    }

    free(u);
    free(v);
    free(tmp);

    return sqrt(vbv / vv);
}

static double get_time_seconds(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec + (double)ts.tv_nsec / 1e9;
}

int main(void) {
    int n = N_SIZE;

    printf("Spectral Norm Benchmark\n");
    printf("N: %d\n", n);

    printf("Warming up (%d runs at n=%d)...\n", WARMUP_RUNS, WARMUP_N);
    for (int w = 0; w < WARMUP_RUNS; w++) {
        double warmup_result = run_spectral_norm(WARMUP_N);
        (void)warmup_result;
    }

    printf("Running measured iterations...\n");
    double min_time = 1e9;
    double result = 0.0;

    for (int iter = 0; iter < ITERATIONS; iter++) {
        double start = get_time_seconds();
        double norm = run_spectral_norm(n);
        double end = get_time_seconds();

        double elapsed = (end - start) * 1000.0;
        if (elapsed < min_time) {
            min_time = elapsed;
            result = norm;
        }
    }

    // 10 iterations * 2 AtAv per iteration * 2 mat-vec per AtAv = 40 mat-vec multiplies
    // Each mat-vec is n*n multiply-adds = 2*n*n FLOPs
    double flops = 40.0 * 2.0 * (double)n * (double)n;
    double mflops = flops / (min_time / 1000.0) / 1e6;

    printf("Spectral norm: %f\n", result);
    printf("Min time: %f ms\n", min_time);
    printf("Throughput: %f MFLOPS\n", mflops);

    return 0;
}
