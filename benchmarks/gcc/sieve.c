// Sieve of Eratosthenes Benchmark
// Simple boolean array implementation
// Matches Seen implementation: limit=10,000,000, 3 warmup + 5 measured iterations

#include <stdio.h>
#include <stdlib.h>
#include <time.h>

#define LIMIT 10000000
#define WARMUP_RUNS 3
#define ITERATIONS 5

typedef struct {
    int *data;
    int length;
    int capacity;
} IntArray;

static IntArray intarray_new(void) {
    IntArray arr;
    arr.capacity = 1024;
    arr.data = (int *)malloc(arr.capacity * sizeof(int));
    arr.length = 0;
    return arr;
}

static void intarray_push(IntArray *arr, int value) {
    if (arr->length >= arr->capacity) {
        arr->capacity *= 2;
        arr->data = (int *)realloc(arr->data, arr->capacity * sizeof(int));
    }
    arr->data[arr->length++] = value;
}

static void intarray_free(IntArray *arr) {
    free(arr->data);
    arr->data = NULL;
    arr->length = 0;
    arr->capacity = 0;
}

static IntArray sieve_of_eratosthenes(int limit) {
    int *is_prime = (int *)malloc((limit + 1) * sizeof(int));
    for (int i = 0; i <= limit; i++) {
        is_prime[i] = 1;
    }

    is_prime[0] = 0;
    is_prime[1] = 0;

    for (int p = 2; (long long)p * p <= limit; p++) {
        if (is_prime[p] != 0) {
            for (int j = p * p; j <= limit; j += p) {
                is_prime[j] = 0;
            }
        }
    }

    IntArray primes = intarray_new();
    for (int idx = 2; idx <= limit; idx++) {
        if (is_prime[idx] != 0) {
            intarray_push(&primes, idx);
        }
    }

    free(is_prime);
    return primes;
}

static long long compute_checksum(const IntArray *primes) {
    long long sum = 0;
    for (int i = 0; i < primes->length; i++) {
        sum += primes->data[i];
    }
    return sum;
}

static double get_time_seconds(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec + (double)ts.tv_nsec / 1e9;
}

int main(void) {
    int limit = LIMIT;

    printf("Sieve of Eratosthenes Benchmark\n");
    printf("Finding primes up to: %d\n", limit);

    printf("Warming up (%d runs)...\n", WARMUP_RUNS);
    for (int w = 0; w < WARMUP_RUNS; w++) {
        IntArray primes_warmup = sieve_of_eratosthenes(limit);
        intarray_free(&primes_warmup);
    }

    printf("Running measured iterations...\n");
    double min_time = 1e9;
    IntArray result_primes = intarray_new();

    for (int i = 0; i < ITERATIONS; i++) {
        double start = get_time_seconds();
        IntArray primes = sieve_of_eratosthenes(limit);
        double end = get_time_seconds();

        double elapsed = (end - start) * 1000.0;
        if (elapsed < min_time) {
            min_time = elapsed;
            intarray_free(&result_primes);
            result_primes = primes;
        } else {
            intarray_free(&primes);
        }
    }

    int prime_count = result_primes.length;
    long long checksum = compute_checksum(&result_primes);

    printf("Prime count: %d\n", prime_count);
    printf("Checksum: %lld\n", checksum);
    printf("Min time: %f ms\n", min_time);

    intarray_free(&result_primes);

    return 0;
}
