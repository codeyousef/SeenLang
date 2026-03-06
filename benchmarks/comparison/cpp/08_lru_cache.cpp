// LRU Cache Benchmark
// C++ version uses std::unordered_map (batteries-included comparison)
// Same algorithm as Seen: hash map + parallel-array doubly-linked list
// 10M ops, capacity 100K, 70% get / 30% put
#include <cstdio>
#include <cstdint>
#include <unordered_map>
#include <vector>
#include <chrono>

static void move_to_front(int64_t nodeIdx, std::vector<int64_t>& prev,
                           std::vector<int64_t>& next,
                           int64_t& head, int64_t& tail) {
    if (nodeIdx == head) return;
    int64_t p = prev[(size_t)nodeIdx];
    int64_t nx = next[(size_t)nodeIdx];
    if (p >= 0) next[(size_t)p] = nx;
    if (nx >= 0) prev[(size_t)nx] = p;
    if (nodeIdx == tail) tail = p;
    prev[(size_t)nodeIdx] = -1;
    next[(size_t)nodeIdx] = head;
    if (head >= 0) prev[(size_t)head] = nodeIdx;
    head = nodeIdx;
}

static int64_t benchmark_lru(int64_t n, int64_t capacity) {
    std::unordered_map<int64_t, int64_t> map;
    map.reserve((size_t)capacity);

    std::vector<int64_t> keys((size_t)capacity, 0);
    std::vector<int64_t> values((size_t)capacity, 0);
    std::vector<int64_t> prev((size_t)capacity, -1);
    std::vector<int64_t> next((size_t)capacity, -1);
    std::vector<int64_t> freeList((size_t)capacity);

    for (int64_t i = 0; i < capacity; i++) freeList[(size_t)i] = i;

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
            auto it = map.find(key);
            if (it != map.end()) {
                int64_t nodeIdx = it->second;
                checksum += values[(size_t)nodeIdx];
                move_to_front(nodeIdx, prev, next, head, tail);
            }
        } else {
            // PUT
            auto it = map.find(key);
            if (it != map.end()) {
                int64_t nodeIdx = it->second;
                values[(size_t)nodeIdx] = key * 2;
                move_to_front(nodeIdx, prev, next, head, tail);
            } else {
                int64_t ni;
                if (freeCount > 0) {
                    freeCount--;
                    ni = freeList[(size_t)freeCount];
                    size++;
                } else {
                    ni = tail;
                    int64_t oldKey = keys[(size_t)ni];
                    map.erase(oldKey);
                    int64_t p = prev[(size_t)ni];
                    tail = p;
                    if (p >= 0) next[(size_t)p] = -1;
                    else head = -1;
                }
                keys[(size_t)ni] = key;
                values[(size_t)ni] = key * 2;
                prev[(size_t)ni] = -1;
                next[(size_t)ni] = head;
                if (head >= 0) prev[(size_t)head] = ni;
                head = ni;
                if (tail < 0) tail = ni;
                map[key] = ni;
            }
        }
    }

    return checksum;
}

int main() {
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
        auto start = std::chrono::high_resolution_clock::now();
        int64_t checksum = benchmark_lru(n, capacity);
        auto end = std::chrono::high_resolution_clock::now();
        double elapsed = std::chrono::duration<double, std::milli>(end - start).count();
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
