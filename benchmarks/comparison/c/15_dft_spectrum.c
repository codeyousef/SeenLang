// DFT Power Spectrum Benchmark (O(N^2))
// N=8192 samples, 3-sine signal, power spectrum + phase
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <time.h>
#include <math.h>

static const double TWO_PI = 2.0 * 3.141592653589793;

static double run_dft_spectrum(int n) {
    int half_n = n / 2;

    // Generate signal: sum of 3 sines
    double* signal = (double*)malloc((size_t)n * sizeof(double));
    for (int i = 0; i < n; i++) {
        double t = (double)i / (double)n;
        signal[i] = 1.0 * sin(TWO_PI * 50.0 * t)
                   + 0.5 * sin(TWO_PI * 120.0 * t)
                   + 0.3 * sin(TWO_PI * 300.0 * t);
    }

    // DFT
    double* re = (double*)malloc((size_t)half_n * sizeof(double));
    double* im = (double*)malloc((size_t)half_n * sizeof(double));

    for (int k = 0; k < half_n; k++) {
        double sum_re = 0.0;
        double sum_im = 0.0;
        for (int j = 0; j < n; j++) {
            double angle = TWO_PI * (double)k * (double)j / (double)n;
            sum_re += signal[j] * cos(angle);
            sum_im -= signal[j] * sin(angle);
        }
        re[k] = sum_re;
        im[k] = sum_im;
    }

    // Power spectrum (dB) and phase
    double checksum = 0.0;
    for (int k = 0; k < half_n; k++) {
        double power = re[k] * re[k] + im[k] * im[k];
        double power_db;
        if (power > 1e-9) {
            power_db = 10.0 * log10(power);
        } else {
            power_db = 0.0;
        }
        double phase = atan2(im[k], re[k]);
        checksum += power_db + phase;
    }

    free(signal);
    free(re);
    free(im);

    return checksum;
}

static double get_time_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000.0 + ts.tv_nsec / 1000000.0;
}

int main(void) {
    int n = 8192;

    printf("DFT Power Spectrum Benchmark\n");
    printf("N: %d samples\n", n);

    printf("Warming up (3 runs at n=512)...\n");
    for (int w = 0; w < 3; w++) {
        (void)run_dft_spectrum(512);
    }

    printf("Running measured iterations...\n");
    int iterations = 5;
    double min_time = 1e18;
    double result_checksum = 0.0;

    for (int i = 0; i < iterations; i++) {
        double start = get_time_ms();
        double checksum = run_dft_spectrum(n);
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
