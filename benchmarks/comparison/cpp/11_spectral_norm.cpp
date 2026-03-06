// Spectral Norm Benchmark
// Same algorithm as Seen: eigenvalue approximation, N=5500, 10 power iterations
#include <cstdio>
#include <cstdint>
#include <cmath>
#include <vector>
#include <chrono>

static inline double eval_a(int i, int j) {
    int ij = i + j;
    return 1.0 / (double)(ij * (ij + 1) / 2 + i + 1);
}

static void multiply_av(const double* v, double* out, int n) {
    for (int i = 0; i < n; i++) {
        double sum = 0.0;
        for (int j = 0; j < n; j++) {
            sum += eval_a(i, j) * v[j];
        }
        out[i] = sum;
    }
}

static void multiply_atv(const double* v, double* out, int n) {
    for (int i = 0; i < n; i++) {
        double sum = 0.0;
        for (int j = 0; j < n; j++) {
            sum += eval_a(j, i) * v[j];
        }
        out[i] = sum;
    }
}

static void multiply_atav(const double* v, double* out, double* tmp, int n) {
    multiply_av(v, tmp, n);
    multiply_atv(tmp, out, n);
}

static double run_spectral_norm(int n) {
    std::vector<double> u((size_t)n, 1.0);
    std::vector<double> v((size_t)n, 0.0);
    std::vector<double> tmp((size_t)n, 0.0);

    for (int iter = 0; iter < 10; iter++) {
        multiply_atav(u.data(), v.data(), tmp.data(), n);
        multiply_atav(v.data(), u.data(), tmp.data(), n);
    }

    double vbv = 0.0, vv = 0.0;
    for (int i = 0; i < n; i++) {
        vbv += u[(size_t)i] * v[(size_t)i];
        vv += v[(size_t)i] * v[(size_t)i];
    }

    return std::sqrt(vbv / vv);
}

int main() {
    int n = 5500;

    printf("Spectral Norm Benchmark\n");
    printf("N: %d\n", n);

    int warmup_runs = 3;
    printf("Warming up (%d runs at n=500)...\n", warmup_runs);
    for (int w = 0; w < warmup_runs; w++) {
        (void)run_spectral_norm(500);
    }

    printf("Running measured iterations...\n");
    int iterations = 5;
    double min_time = 1e18;
    double result = 0.0;

    for (int iter = 0; iter < iterations; iter++) {
        auto start = std::chrono::high_resolution_clock::now();
        double norm = run_spectral_norm(n);
        auto end = std::chrono::high_resolution_clock::now();
        double elapsed = std::chrono::duration<double, std::milli>(end - start).count();
        if (elapsed < min_time) {
            min_time = elapsed;
            result = norm;
        }
    }

    double flops = 40.0 * 2.0 * (double)n * (double)n;
    double mflops = flops / (min_time / 1000.0) / 1e6;

    printf("Spectral norm: %.9f\n", result);
    printf("Min time: %.6f ms\n", min_time);
    printf("Throughput: %.6f MFLOPS\n", mflops);
    return 0;
}
