// Sieve of Eratosthenes Benchmark
// Same algorithm as Seen: int64_t flag array, limit=10000000
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <time.h>

typedef struct {
    int64_t* data;
    int64_t length;
    int64_t capacity;
} IntVec;

static IntVec intvec_new(void) {
    IntVec v = { NULL, 0, 0 };
    return v;
}

static void intvec_push(IntVec* v, int64_t val) {
    if (v->length == v->capacity) {
        v->capacity = v->capacity == 0 ? 256 : v->capacity * 2;
        v->data = (int64_t*)realloc(v->data, (size_t)v->capacity * sizeof(int64_t));
    }
    v->data[v->length++] = val;
}

static void intvec_free(IntVec* v) {
    free(v->data);
    v->data = NULL;
    v->length = v->capacity = 0;
}

static IntVec sieve_of_eratosthenes(int64_t limit) {
    int64_t* is_prime = (int64_t*)malloc((size_t)(limit + 1) * sizeof(int64_t));
    for (int64_t i = 0; i <= limit; i++) is_prime[i] = 1;
    is_prime[0] = 0;
    is_prime[1] = 0;

    for (int64_t p = 2; p * p <= limit; p++) {
        if (is_prime[p] != 0) {
            for (int64_t j = p * p; j <= limit; j += p) {
                is_prime[j] = 0;
            }
        }
    }

    IntVec primes = intvec_new();
    for (int64_t idx = 2; idx <= limit; idx++) {
        if (is_prime[idx] != 0) {
            intvec_push(&primes, idx);
        }
    }

    free(is_prime);
    return primes;
}

static int64_t compute_checksum(const IntVec* primes) {
    int64_t sum = 0;
    for (int64_t i = 0; i < primes->length; i++) {
        sum += primes->data[i];
    }
    return sum;
}

static double get_time_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000.0 + ts.tv_nsec / 1000000.0;
}

int main(void) {
    int64_t limit = 10000000;

    printf("Sieve of Eratosthenes Benchmark\n");
    printf("Finding primes up to: %ld\n", (long)limit);

    int warmup_runs = 3;
    printf("Warming up (%d runs)...\n", warmup_runs);
    for (int w = 0; w < warmup_runs; w++) {
        IntVec p = sieve_of_eratosthenes(limit);
        intvec_free(&p);
    }

    printf("Running measured iterations...\n");
    int iterations = 5;
    double min_time = 1e18;
    IntVec result_primes = intvec_new();

    for (int i = 0; i < iterations; i++) {
        double start = get_time_ms();
        IntVec primes = sieve_of_eratosthenes(limit);
        double elapsed = get_time_ms() - start;

        if (elapsed < min_time) {
            min_time = elapsed;
            intvec_free(&result_primes);
            result_primes = primes;
        } else {
            intvec_free(&primes);
        }
    }

    int64_t prime_count = result_primes.length;
    int64_t checksum = compute_checksum(&result_primes);

    printf("Prime count: %ld\n", (long)prime_count);
    printf("Checksum: %ld\n", (long)checksum);
    printf("Min time: %.6f ms\n", min_time);

    intvec_free(&result_primes);
    return 0;
}
