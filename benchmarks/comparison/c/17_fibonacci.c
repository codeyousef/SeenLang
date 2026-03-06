// Fibonacci Benchmark
// Compute sum of first N fibonacci numbers using iterative method
// Tests loop performance and integer arithmetic
// N = 80000000, expected checksum depends on mod arithmetic (i64 overflow wraps)
#include <stdio.h>
#include <stdint.h>
#include <time.h>

static int64_t benchmark_fib(int64_t n) {
    int64_t a = 0, b = 1, sum = 0;
    for (int64_t i = 0; i < n; i++) {
        sum += a;
        int64_t c = a + b;
        a = b;
        b = c;
    }
    return sum;
}

static double get_time_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000.0 + ts.tv_nsec / 1000000.0;
}

int main(void) {
    int64_t n = 1000000000;

    printf("Fibonacci Benchmark\n");
    printf("Computing sum of first %ld fibonacci numbers\n", (long)n);

    int warmup_runs = 3;
    printf("Warming up (%d runs)...\n", warmup_runs);
    for (int w = 0; w < warmup_runs; w++) {
        volatile int64_t r = benchmark_fib(n / 10);
        (void)r;
    }

    printf("Running measured iterations...\n");
    int iterations = 5;
    double min_time = 1e18;
    int64_t result = 0;

    for (int i = 0; i < iterations; i++) {
        double start = get_time_ms();
        int64_t answer = benchmark_fib(n);
        double elapsed = get_time_ms() - start;
        if (elapsed < min_time) {
            min_time = elapsed;
            result = answer;
        }
    }

    printf("Checksum: %ld\n", (long)result);
    printf("Min time: %.6f ms\n", min_time);
    return 0;
}
