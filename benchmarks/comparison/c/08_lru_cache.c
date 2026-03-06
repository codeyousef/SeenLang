// LRU Cache Benchmark
// Same algorithm as Seen: open-addressing hash map + parallel-array doubly-linked list
// 10M ops, capacity 100K, 70% get / 30% put
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <time.h>

// Open-addressing hash map (same algorithm as Seen runtime)
typedef struct {
    int64_t* ht_keys;
    int64_t* ht_vals;   // node index
    int8_t*  ht_flags;  // 0=empty, 1=occupied, 2=tombstone
    int64_t  ht_cap;
    int64_t  ht_size;
    int64_t  ht_tombstones;
} HashMap;

static void hm_init(HashMap* m, int64_t cap) {
    // Round up to power of 2
    int64_t c = 1;
    while (c < cap * 2) c <<= 1;
    m->ht_cap = c;
    m->ht_size = 0;
    m->ht_tombstones = 0;
    m->ht_keys = (int64_t*)calloc((size_t)c, sizeof(int64_t));
    m->ht_vals = (int64_t*)calloc((size_t)c, sizeof(int64_t));
    m->ht_flags = (int8_t*)calloc((size_t)c, sizeof(int8_t));
}

static void hm_free(HashMap* m) {
    free(m->ht_keys); free(m->ht_vals); free(m->ht_flags);
}

static inline int64_t hm_hash(int64_t key, int64_t mask) {
    uint64_t h = (uint64_t)key;
    h ^= h >> 33;
    h *= 0xff51afd7ed558ccdULL;
    h ^= h >> 33;
    h *= 0xc4ceb9fe1a85ec53ULL;
    h ^= h >> 33;
    return (int64_t)(h & (uint64_t)mask);
}

static int hm_get(const HashMap* m, int64_t key, int64_t* out_val) {
    int64_t mask = m->ht_cap - 1;
    int64_t idx = hm_hash(key, mask);
    for (;;) {
        if (m->ht_flags[idx] == 0) return 0; // empty
        if (m->ht_flags[idx] == 1 && m->ht_keys[idx] == key) {
            *out_val = m->ht_vals[idx];
            return 1;
        }
        idx = (idx + 1) & mask;
    }
}

static void hm_grow(HashMap* m);

static void hm_insert(HashMap* m, int64_t key, int64_t val) {
    if ((m->ht_size + m->ht_tombstones) * 10 >= m->ht_cap * 7) hm_grow(m);
    int64_t mask = m->ht_cap - 1;
    int64_t idx = hm_hash(key, mask);
    for (;;) {
        if (m->ht_flags[idx] == 0) {
            m->ht_keys[idx] = key;
            m->ht_vals[idx] = val;
            m->ht_flags[idx] = 1;
            m->ht_size++;
            return;
        }
        if (m->ht_flags[idx] == 2) {
            m->ht_keys[idx] = key;
            m->ht_vals[idx] = val;
            m->ht_flags[idx] = 1;
            m->ht_size++;
            m->ht_tombstones--;
            return;
        }
        if (m->ht_flags[idx] == 1 && m->ht_keys[idx] == key) {
            m->ht_vals[idx] = val;
            return;
        }
        idx = (idx + 1) & mask;
    }
}

static void hm_remove(HashMap* m, int64_t key) {
    int64_t mask = m->ht_cap - 1;
    int64_t idx = hm_hash(key, mask);
    for (;;) {
        if (m->ht_flags[idx] == 0) return;
        if (m->ht_flags[idx] == 1 && m->ht_keys[idx] == key) {
            m->ht_flags[idx] = 2; // tombstone
            m->ht_size--;
            m->ht_tombstones++;
            return;
        }
        idx = (idx + 1) & mask;
    }
}

static void hm_grow(HashMap* m) {
    int64_t old_cap = m->ht_cap;
    int64_t* old_keys = m->ht_keys;
    int64_t* old_vals = m->ht_vals;
    int8_t* old_flags = m->ht_flags;

    m->ht_cap = old_cap * 2;
    m->ht_size = 0;
    m->ht_tombstones = 0;
    m->ht_keys = (int64_t*)calloc((size_t)m->ht_cap, sizeof(int64_t));
    m->ht_vals = (int64_t*)calloc((size_t)m->ht_cap, sizeof(int64_t));
    m->ht_flags = (int8_t*)calloc((size_t)m->ht_cap, sizeof(int8_t));

    for (int64_t i = 0; i < old_cap; i++) {
        if (old_flags[i] == 1) {
            hm_insert(m, old_keys[i], old_vals[i]);
        }
    }
    free(old_keys); free(old_vals); free(old_flags);
}

static void move_to_front(int64_t nodeIdx, int64_t* prev, int64_t* next,
                           int64_t* head, int64_t* tail) {
    if (nodeIdx == *head) return;
    int64_t p = prev[nodeIdx];
    int64_t nx = next[nodeIdx];
    if (p >= 0) next[p] = nx;
    if (nx >= 0) prev[nx] = p;
    if (nodeIdx == *tail) *tail = p;
    prev[nodeIdx] = -1;
    next[nodeIdx] = *head;
    if (*head >= 0) prev[*head] = nodeIdx;
    *head = nodeIdx;
}

static int64_t benchmark_lru(int64_t n, int64_t capacity) {
    HashMap map;
    hm_init(&map, capacity);

    int64_t* keys = (int64_t*)calloc((size_t)capacity, sizeof(int64_t));
    int64_t* values = (int64_t*)calloc((size_t)capacity, sizeof(int64_t));
    int64_t* prev = (int64_t*)malloc((size_t)capacity * sizeof(int64_t));
    int64_t* next = (int64_t*)malloc((size_t)capacity * sizeof(int64_t));
    int64_t* freeList = (int64_t*)malloc((size_t)capacity * sizeof(int64_t));

    for (int64_t i = 0; i < capacity; i++) {
        prev[i] = -1;
        next[i] = -1;
        freeList[i] = i;
    }

    int64_t freeCount = capacity;
    int64_t head = -1, tail = -1;
    int64_t size = 0;
    int64_t checksum = 0;
    int64_t rng = 42;

    for (int64_t i = 0; i < n; i++) {
        rng = (int64_t)((uint64_t)rng * 1103515245ULL + 12345ULL);
        int64_t key = rng < 0 ? -rng : rng;
        int64_t rem2 = i - (i / 10) * 10;

        if (rem2 < 7) {
            // GET
            int64_t nodeIdx;
            if (hm_get(&map, key, &nodeIdx)) {
                checksum += values[nodeIdx];
                move_to_front(nodeIdx, prev, next, &head, &tail);
            }
        } else {
            // PUT
            int64_t nodeIdx;
            if (hm_get(&map, key, &nodeIdx)) {
                values[nodeIdx] = key * 2;
                move_to_front(nodeIdx, prev, next, &head, &tail);
            } else {
                int64_t ni;
                if (freeCount > 0) {
                    freeCount--;
                    ni = freeList[freeCount];
                    size++;
                } else {
                    ni = tail;
                    int64_t oldKey = keys[ni];
                    hm_remove(&map, oldKey);
                    int64_t p = prev[ni];
                    tail = p;
                    if (p >= 0) next[p] = -1;
                    else head = -1;
                }
                keys[ni] = key;
                values[ni] = key * 2;
                prev[ni] = -1;
                next[ni] = head;
                if (head >= 0) prev[head] = ni;
                head = ni;
                if (tail < 0) tail = ni;
                hm_insert(&map, key, ni);
            }
        }
    }

    hm_free(&map);
    free(keys); free(values); free(prev); free(next); free(freeList);
    return checksum;
}

static double get_time_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000.0 + ts.tv_nsec / 1000000.0;
}

int main(void) {
    int64_t n = 10000000;
    int64_t capacity = 100000;

    printf("LRU Cache Benchmark\n");
    printf("Operations: %ld\n", (long)n);
    printf("Capacity: %ld\n", (long)capacity);

    int warmup_runs = 3;
    printf("Warming up (%d runs)...\n", warmup_runs);
    for (int w = 0; w < warmup_runs; w++) {
        (void)benchmark_lru(n / 10, capacity);
    }

    printf("Running measured iterations...\n");
    int iterations = 5;
    double min_time = 1e18;
    int64_t result_checksum = 0;

    for (int i = 0; i < iterations; i++) {
        double start = get_time_ms();
        int64_t checksum = benchmark_lru(n, capacity);
        double elapsed = get_time_ms() - start;
        if (elapsed < min_time) {
            min_time = elapsed;
            result_checksum = checksum;
        }
    }

    printf("Checksum: %ld\n", (long)result_checksum);
    printf("Min time: %.6f ms\n", min_time);
    printf("Operations per second: %.6f million\n", (double)n / (min_time / 1000.0) / 1e6);
    return 0;
}
