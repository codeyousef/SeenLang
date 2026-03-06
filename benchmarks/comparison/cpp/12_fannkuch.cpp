// Fannkuch-Redux Benchmark
// Same algorithm as Seen: Heap's algorithm variant for permutation generation
// Uses int64_t for perm arrays (matching Seen's Int type)
// N=12
#include <cstdio>
#include <cstdint>
#include <vector>
#include <chrono>

static void run_fannkuch(int n, int64_t& out_checksum, int64_t& out_max_flips) {
    std::vector<int64_t> perm((size_t)n);
    std::vector<int64_t> perm1((size_t)n);
    std::vector<int64_t> count((size_t)n);

    for (int i = 0; i < n; i++) perm1[(size_t)i] = (int64_t)i;

    int64_t max_flips = 0;
    int64_t checksum = 0;
    int64_t perm_count = 0;
    int r = n;

    for (;;) {
        while (r != 1) {
            count[(size_t)(r - 1)] = (int64_t)r;
            r--;
        }

        for (int i = 0; i < n; i++) perm[(size_t)i] = perm1[(size_t)i];

        int64_t flips = 0;
        int k = perm[0];
        while (k != 0) {
            int lo = 0, hi = k;
            while (lo < hi) {
                std::swap(perm[(size_t)lo], perm[(size_t)hi]);
                lo++; hi--;
            }
            flips++;
            k = perm[0];
        }

        if (flips > max_flips) max_flips = flips;
        if (perm_count % 2 == 0) checksum += flips;
        else checksum -= flips;
        perm_count++;

        r = 1;
        for (;;) {
            if (r >= n) {
                out_checksum = checksum;
                out_max_flips = max_flips;
                return;
            }
            int64_t perm0 = perm1[0];
            for (int i = 0; i < r; i++) perm1[(size_t)i] = perm1[(size_t)(i + 1)];
            perm1[(size_t)r] = perm0;
            count[(size_t)r]--;
            if (count[(size_t)r] > 0) break;
            r++;
        }
    }
}

int main() {
    int n = 12;

    printf("Fannkuch-Redux Benchmark\n");
    printf("N: %d\n", n);

    printf("Warming up (1 run at n=10)...\n");
    int64_t wc, wf;
    run_fannkuch(10, wc, wf);

    printf("Running measured iterations...\n");
    int iterations = 3;
    double min_time = 1e18;
    int64_t result_checksum = 0;
    int64_t result_max_flips = 0;

    for (int iter = 0; iter < iterations; iter++) {
        int64_t cs, mf;
        auto start = std::chrono::high_resolution_clock::now();
        run_fannkuch(n, cs, mf);
        auto end = std::chrono::high_resolution_clock::now();
        double elapsed = std::chrono::duration<double, std::milli>(end - start).count();
        if (elapsed < min_time) {
            min_time = elapsed;
            result_checksum = cs;
            result_max_flips = mf;
        }
    }

    printf("Checksum: %ld\n", (long)result_checksum);
    printf("Max flips: %ld\n", (long)result_max_flips);
    printf("Min time: %.6f ms\n", min_time);
    return 0;
}
