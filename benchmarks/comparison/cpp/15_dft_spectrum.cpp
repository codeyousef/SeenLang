// Discrete Fourier Transform (Power Spectrum) Benchmark
// Same algorithm as Seen: O(N^2) naive DFT, power spectrum in dB + phase
// N=8192, signal = 3 sine waves (50, 120, 300 Hz)
#include <cstdio>
#include <cstdint>
#include <cmath>
#include <vector>
#include <chrono>

static double run_dft(int n) {
    double pi = 3.141592653589793;
    double two_pi = 6.283185307179586;

    // Generate synthetic signal: sum of 3 sine waves
    std::vector<double> signal((size_t)n);
    for (int i = 0; i < n; i++) {
        double t = (double)i / (double)n;
        signal[(size_t)i] = 1.0 * std::sin(two_pi * 50.0 * t)
                          + 0.5 * std::sin(two_pi * 120.0 * t)
                          + 0.3 * std::sin(two_pi * 300.0 * t);
    }

    // Compute DFT: X[k] = sum_{n=0}^{N-1} x[n] * exp(-j*2pi*k*n/N)
    int half_n = n / 2;
    std::vector<double> dft_re((size_t)half_n, 0.0);
    std::vector<double> dft_im((size_t)half_n, 0.0);

    for (int k = 0; k < half_n; k++) {
        double re_sum = 0.0;
        double im_sum = 0.0;
        for (int j = 0; j < n; j++) {
            double angle = two_pi * (double)k * (double)j / (double)n;
            re_sum += signal[(size_t)j] * std::cos(angle);
            im_sum -= signal[(size_t)j] * std::sin(angle);
        }
        dft_re[(size_t)k] = re_sum;
        dft_im[(size_t)k] = im_sum;
    }

    // Compute power spectrum in dB and phase
    double checksum = 0.0;
    for (int m = 0; m < half_n; m++) {
        double power = dft_re[(size_t)m] * dft_re[(size_t)m]
                     + dft_im[(size_t)m] * dft_im[(size_t)m];
        double power_db = 0.0;
        if (power > 1e-9) {
            power_db = 10.0 * std::log10(power);
        }
        double phase = std::atan2(dft_im[(size_t)m], dft_re[(size_t)m]);
        checksum += power_db + phase;
    }

    return checksum;
}

int main() {
    int n = 8192;

    printf("DFT Power Spectrum Benchmark\n");
    printf("Signal length: %d\n", n);

    int warmup_runs = 3;
    printf("Warming up (%d runs at n=512)...\n", warmup_runs);
    for (int w = 0; w < warmup_runs; w++) {
        (void)run_dft(512);
    }

    printf("Running measured iterations...\n");
    int iterations = 5;
    double min_time = 1e18;
    double result = 0.0;

    for (int iter = 0; iter < iterations; iter++) {
        auto start = std::chrono::high_resolution_clock::now();
        double checksum = run_dft(n);
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
