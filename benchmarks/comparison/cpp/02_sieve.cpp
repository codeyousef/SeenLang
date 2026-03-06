// Sieve of Eratosthenes Benchmark
// Same algorithm as Seen: int64_t flag array, limit=10000000
#include <cstdio>
#include <cstdint>
#include <vector>
#include <chrono>

static std::vector<int64_t> sieve_of_eratosthenes(int64_t limit) {
    std::vector<int64_t> is_prime((size_t)(limit + 1), 1);
    is_prime[0] = 0;
    is_prime[1] = 0;

    for (int64_t p = 2; p * p <= limit; p++) {
        if (is_prime[(size_t)p] != 0) {
            for (int64_t j = p * p; j <= limit; j += p) {
                is_prime[(size_t)j] = 0;
            }
        }
    }

    std::vector<int64_t> primes;
    for (int64_t idx = 2; idx <= limit; idx++) {
        if (is_prime[(size_t)idx] != 0) {
            primes.push_back(idx);
        }
    }
    return primes;
}

static int64_t compute_checksum(const std::vector<int64_t>& primes) {
    int64_t sum = 0;
    for (auto p : primes) sum += p;
    return sum;
}

int main() {
    int64_t limit = 10000000;

    printf("Sieve of Eratosthenes Benchmark\n");
    printf("Finding primes up to: %ld\n", (long)limit);

    int warmup_runs = 3;
    printf("Warming up (%d runs)...\n", warmup_runs);
    for (int w = 0; w < warmup_runs; w++) {
        auto p = sieve_of_eratosthenes(limit);
    }

    printf("Running measured iterations...\n");
    int iterations = 5;
    double min_time = 1e18;
    std::vector<int64_t> result_primes;

    for (int i = 0; i < iterations; i++) {
        auto start = std::chrono::high_resolution_clock::now();
        auto primes = sieve_of_eratosthenes(limit);
        auto end = std::chrono::high_resolution_clock::now();
        double elapsed = std::chrono::duration<double, std::milli>(end - start).count();

        if (elapsed < min_time) {
            min_time = elapsed;
            result_primes = std::move(primes);
        }
    }

    int64_t prime_count = (int64_t)result_primes.size();
    int64_t checksum = compute_checksum(result_primes);

    printf("Prime count: %ld\n", (long)prime_count);
    printf("Checksum: %ld\n", (long)checksum);
    printf("Min time: %.6f ms\n", min_time);
    return 0;
}
