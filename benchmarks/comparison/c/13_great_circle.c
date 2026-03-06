// Great Circle (Haversine) Distance Benchmark
// N=2,000,000 pairs, earth_radius=6371.0
// Golden ratio spiral point generation
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <time.h>
#include <math.h>

static const double EARTH_RADIUS = 6371.0;
static const double PHI = 1.618033988749895;
static const double PI = 3.141592653589793;

static double haversine(double lat1, double lon1, double lat2, double lon2) {
    double half_dlat = (lat2 - lat1) / 2.0;
    double half_dlon = (lon2 - lon1) / 2.0;
    double a = sin(half_dlat) * sin(half_dlat)
             + cos(lat1) * cos(lat2) * sin(half_dlon) * sin(half_dlon);
    return EARTH_RADIUS * 2.0 * asin(sqrt(a));
}

static double run_great_circle(int n) {
    double total_distance = 0.0;
    double two_pi = 2.0 * PI;

    for (int i = 0; i < n; i++) {
        double t1 = (double)i / (double)n;
        double lat1 = asin(2.0 * t1 - 1.0);
        double lon1 = two_pi * PHI * (double)i;

        int j = i + 1;
        double t2 = (double)j / (double)n;
        double lat2 = asin(2.0 * t2 - 1.0);
        double lon2 = two_pi * PHI * (double)j;

        total_distance += haversine(lat1, lon1, lat2, lon2);
    }

    return total_distance;
}

static double get_time_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000.0 + ts.tv_nsec / 1000000.0;
}

int main(void) {
    int n = 2000000;

    printf("Great Circle (Haversine) Distance Benchmark\n");
    printf("N: %d pairs\n", n);

    printf("Warming up (3 runs at n=40000)...\n");
    for (int w = 0; w < 3; w++) {
        (void)run_great_circle(40000);
    }

    printf("Running measured iterations...\n");
    int iterations = 5;
    double min_time = 1e18;
    double result_checksum = 0.0;

    for (int i = 0; i < iterations; i++) {
        double start = get_time_ms();
        double checksum = run_great_circle(n);
        double elapsed = get_time_ms() - start;
        if (elapsed < min_time) {
            min_time = elapsed;
            result_checksum = checksum;
        }
    }

    printf("Checksum: %.6f\n", result_checksum);
    printf("Min time: %.6f ms\n", min_time);
    return 0;
}
