// Great-Circle Distance (Haversine) Benchmark
// Same algorithm as Seen: golden spiral points, haversine formula
// N=2,000,000, R=6371.0
#include <cstdio>
#include <cstdint>
#include <cmath>
#include <chrono>

static double haversine(double lat1, double lon1, double lat2, double lon2) {
    double earth_radius = 6371.0;
    double half_dlat = (lat2 - lat1) / 2.0;
    double half_dlon = (lon2 - lon1) / 2.0;
    double a = std::sin(half_dlat) * std::sin(half_dlat)
             + std::cos(lat1) * std::cos(lat2)
             * std::sin(half_dlon) * std::sin(half_dlon);
    return earth_radius * 2.0 * std::asin(std::sqrt(a));
}

static double run_great_circle(int64_t n) {
    double pi = 3.141592653589793;
    double golden_ratio = 1.618033988749895;
    double total_distance = 0.0;

    for (int64_t i = 0; i < n; i++) {
        // Point 1 from golden spiral
        double t1 = (double)i / (double)n;
        double lat1 = std::asin(2.0 * t1 - 1.0);
        double lon1 = 2.0 * pi * golden_ratio * (double)i;

        // Point 2 from offset golden spiral
        int64_t j = i + 1;
        double t2 = (double)j / (double)n;
        double lat2 = std::asin(2.0 * t2 - 1.0);
        double lon2 = 2.0 * pi * golden_ratio * (double)j;

        double dist = haversine(lat1, lon1, lat2, lon2);
        total_distance += dist;
    }

    return total_distance;
}

int main() {
    int64_t n = 2000000;

    printf("Great-Circle Distance (Haversine) Benchmark\n");
    printf("Pairs: %ld\n", (long)n);

    int warmup_runs = 3;
    printf("Warming up (%d runs at n=40000)...\n", warmup_runs);
    for (int w = 0; w < warmup_runs; w++) {
        (void)run_great_circle(40000);
    }

    printf("Running measured iterations...\n");
    int iterations = 5;
    double min_time = 1e18;
    double result = 0.0;

    for (int iter = 0; iter < iterations; iter++) {
        auto start = std::chrono::high_resolution_clock::now();
        double checksum = run_great_circle(n);
        auto end = std::chrono::high_resolution_clock::now();
        double elapsed = std::chrono::duration<double, std::milli>(end - start).count();
        if (elapsed < min_time) {
            min_time = elapsed;
            result = checksum;
        }
    }

    printf("Checksum: %.6f\n", result);
    printf("Min time: %.6f ms\n", min_time);
    return 0;
}
