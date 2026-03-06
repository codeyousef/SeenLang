// Euler Totient via GCD Benchmark
// N=10000, sum of euler_totient(n) for n=1..N
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <time.h>

static int64_t gcd(int64_t a, int64_t b) {
    while (b > 0) {
        int64_t temp = b;
        b = a % b;
        a = temp;
    }
    return a;
}

static int64_t euler_totient(int64_t n) {
    int64_t count = 0;
    for (int64_t k = 1; k < n; k++) {
        if (gcd(k, n) == 1) {
            count++;
        }
    }
    return count;
}

static int64_t run_totient_sum(int64_t limit) {
    int64_t sum = 0;
    for (int64_t n = 1; n <= limit; n++) {
        sum += euler_totient(n);
    }
    return sum;
}

static double get_time_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000.0 + ts.tv_nsec / 1000000.0;
}

int main(void) {
    int64_t n = 10000;

    printf("Euler Totient via GCD Benchmark\n");
    printf("N: %ld\n", (long)n);

    printf("Warming up (3 runs at n=1000)...\n");
    for (int w = 0; w < 3; w++) {
        (void)run_totient_sum(1000);
    }

    printf("Running measured iterations...\n");
    int iterations = 5;
    double min_time = 1e18;
    int64_t result_checksum = 0;

    for (int i = 0; i < iterations; i++) {
        double start = get_time_ms();
        int64_t checksum = run_totient_sum(n);
        double elapsed = get_time_ms() - start;
        if (elapsed < min_time) {
            min_time = elapsed;
            result_checksum = checksum;
        }
    }

    printf("Checksum: %ld\n", (long)result_checksum);
    printf("Min time: %.6f ms\n", min_time);
    return 0;
}
