// Mandelbrot Set Benchmark
// Same algorithm as Seen: escape-time, 1000x1000, max_iter=100, bailout=4.0
// Domain: [-2.5, 1.0] x [-1.0, 1.0]
#include <cstdio>
#include <cstdint>
#include <vector>
#include <chrono>

static int mandelbrot_pixel(double cx, double cy, int max_iter) {
    double zx = 0.0;
    double zy = 0.0;
    int iteration = 0;

    while (iteration < max_iter) {
        double zx2 = zx * zx;
        double zy2 = zy * zy;

        if (zx2 + zy2 > 4.0) {
            return iteration;
        }

        double new_zy = 2.0 * zx * zy + cy;
        zx = zx2 - zy2 + cx;
        zy = new_zy;

        iteration++;
    }

    return iteration;
}

static std::vector<int> compute_mandelbrot(int width, int height, int max_iter) {
    std::vector<int> pixels;
    pixels.reserve((size_t)width * (size_t)height);

    double x_min = -2.5;
    double x_max = 1.0;
    double y_min = -1.0;
    double y_max = 1.0;

    double x_scale = (x_max - x_min) / (double)width;
    double y_scale = (y_max - y_min) / (double)height;

    for (int y = 0; y < height; y++) {
        for (int x = 0; x < width; x++) {
            double cx = x_min + (double)x * x_scale;
            double cy = y_min + (double)y * y_scale;
            int iter = mandelbrot_pixel(cx, cy, max_iter);
            pixels.push_back(iter);
        }
    }

    return pixels;
}

static int64_t compute_checksum(const std::vector<int>& pixels) {
    int64_t sum = 0;
    for (size_t i = 0; i < pixels.size(); i++) {
        sum += pixels[i];
    }
    return sum;
}

int main() {
    int width = 1000;
    int height = 1000;
    int max_iter = 100;

    printf("Mandelbrot Set Benchmark\n");
    printf("Image size: %dx%d\n", width, height);
    printf("Max iterations: %d\n", max_iter);

    int warmup_runs = 3;
    printf("Warming up (%d runs at 250x250)...\n", warmup_runs);
    for (int w = 0; w < warmup_runs; w++) {
        auto pixels_warmup = compute_mandelbrot(250, 250, max_iter);
    }

    printf("Running measured iterations...\n");
    int iterations = 5;
    double min_time = 1e18;
    int64_t result_checksum = 0;

    for (int i = 0; i < iterations; i++) {
        auto start = std::chrono::high_resolution_clock::now();
        auto pixels = compute_mandelbrot(width, height, max_iter);
        auto end = std::chrono::high_resolution_clock::now();
        double elapsed = std::chrono::duration<double, std::milli>(end - start).count();
        if (elapsed < min_time) {
            min_time = elapsed;
            result_checksum = compute_checksum(pixels);
        }
    }

    printf("Checksum: %ld\n", (long)result_checksum);
    printf("Min time: %.6f ms\n", min_time);
    return 0;
}
