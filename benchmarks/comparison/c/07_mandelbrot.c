// Mandelbrot Escape-Time Benchmark
// 1000x1000, max_iter=100, bailout=4.0
// Domain: x in [-2.5, 1.0], y in [-1.0, 1.0]
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <time.h>

static int mandelbrot_pixel(double cx, double cy, int max_iter) {
    double zr = 0.0, zi = 0.0;
    for (int i = 0; i < max_iter; i++) {
        double zr2 = zr * zr;
        double zi2 = zi * zi;
        if (zr2 + zi2 > 4.0) return i;
        zi = 2.0 * zr * zi + cy;
        zr = zr2 - zi2 + cx;
    }
    return max_iter;
}

static int64_t run_mandelbrot(int width, int height, int max_iter) {
    int64_t checksum = 0;
    double x_min = -2.5, x_max = 1.0;
    double y_min = -1.0, y_max = 1.0;

    for (int py = 0; py < height; py++) {
        double cy = y_min + (double)py / (double)height * (y_max - y_min);
        for (int px = 0; px < width; px++) {
            double cx = x_min + (double)px / (double)width * (x_max - x_min);
            int iters = mandelbrot_pixel(cx, cy, max_iter);
            checksum += (int64_t)iters;
        }
    }

    return checksum;
}

static double get_time_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000.0 + ts.tv_nsec / 1000000.0;
}

int main(void) {
    int width = 1000;
    int height = 1000;
    int max_iter = 100;

    printf("Mandelbrot Escape-Time Benchmark\n");
    printf("Size: %dx%d, max_iter: %d\n", width, height, max_iter);

    printf("Warming up (3 runs at 250x250)...\n");
    for (int w = 0; w < 3; w++) {
        (void)run_mandelbrot(250, 250, max_iter);
    }

    printf("Running measured iterations...\n");
    int iterations = 5;
    double min_time = 1e18;
    int64_t result_checksum = 0;

    for (int i = 0; i < iterations; i++) {
        double start = get_time_ms();
        int64_t checksum = run_mandelbrot(width, height, max_iter);
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
