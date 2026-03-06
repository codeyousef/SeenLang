// Euler Totient (Number Theory) Benchmark
// Same algorithm as Seen: brute-force GCD for phi(n), n=1..10000
// Pure integer math — no hardware intrinsic advantage
#include <cstdio>
#include <cstdint>
#include <chrono>

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
    int64_t total = 0;
    for (int64_t n = 1; n <= limit; n++) {
        total += euler_totient(n);
    }
    return total;
}

int main() {
    int64_t n = 10000;

    printf("Euler Totient (Number Theory) Benchmark\n");
    printf("Computing phi(1) to phi(%ld)\n", (long)n);

    int warmup_runs = 3;
    printf("Warming up (%d runs at n=1000)...\n", warmup_runs);
    for (int w = 0; w < warmup_runs; w++) {
        (void)run_totient_sum(1000);
    }

    printf("Running measured iterations...\n");
    int iterations = 5;
    double min_time = 1e18;
    int64_t result = 0;

    for (int iter = 0; iter < iterations; iter++) {
        auto start = std::chrono::high_resolution_clock::now();
        int64_t checksum = run_totient_sum(n);
        auto end = std::chrono::high_resolution_clock::now();
        double elapsed = std::chrono::duration<double, std::milli>(end - start).count();
        if (elapsed < min_time) {
            min_time = elapsed;
            result = checksum;
        }
    }

    printf("Checksum: %ld\n", (long)result);
    printf("Min time: %.6f ms\n", min_time);
    return 0;
}
