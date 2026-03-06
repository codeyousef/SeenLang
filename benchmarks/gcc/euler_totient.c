// Euler Totient (Number Theory) Benchmark
// Computes Euler's totient function phi(n) for n = 1..N using GCD
// Matches Seen implementation: n=10000, 3 warmup + 5 measured iterations

#include <stdio.h>
#include <time.h>

#define N_LIMIT 10000
#define WARMUP_RUNS 3
#define WARMUP_N 1000
#define ITERATIONS 5

static inline long long seen_gcd(long long a, long long b) {
    long long x = a;
    long long y = b;
    while (y > 0) {
        long long temp = y;
        y = x % y;
        x = temp;
    }
    return x;
}

static long long euler_totient(long long n) {
    long long count = 0;
    for (long long k = 1; k < n; k++) {
        if (seen_gcd(k, n) == 1) {
            count++;
        }
    }
    return count;
}

static long long run_totient_sum(long long limit) {
    long long total = 0;
    for (long long n = 1; n <= limit; n++) {
        total += euler_totient(n);
    }
    return total;
}

static double get_time_seconds(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec + (double)ts.tv_nsec / 1e9;
}

int main(void) {
    long long n = N_LIMIT;

    printf("Euler Totient (Number Theory) Benchmark\n");
    printf("Computing phi(1) to phi(%lld)\n", n);

    printf("Warming up (%d runs at n=%d)...\n", WARMUP_RUNS, WARMUP_N);
    for (int w = 0; w < WARMUP_RUNS; w++) {
        long long warmup_result = run_totient_sum(WARMUP_N);
        (void)warmup_result;
    }

    printf("Running measured iterations...\n");
    double min_time = 1e9;
    long long result = 0;

    for (int iter = 0; iter < ITERATIONS; iter++) {
        double start = get_time_seconds();
        long long checksum = run_totient_sum(n);
        double end = get_time_seconds();

        double elapsed = (end - start) * 1000.0;
        if (elapsed < min_time) {
            min_time = elapsed;
            result = checksum;
        }
    }

    printf("Sum of phi(1..N): %lld\n", result);
    printf("Min time: %f ms\n", min_time);
    printf("Totients per second: %f million\n",
           (double)n / (min_time / 1000.0) / 1e6);

    return 0;
}
