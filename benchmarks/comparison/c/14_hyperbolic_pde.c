// Hyperbolic PDE (1D Catenary Cable) Benchmark
// grid_size=10000, timesteps=500, dx=4.0/grid_size, dt=0.0001
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <time.h>
#include <math.h>

static double run_hyperbolic_pde(int grid_size, int timesteps) {
    double dx = 4.0 / (double)grid_size;
    double dt = 0.0001;

    double* u = (double*)malloc((size_t)grid_size * sizeof(double));
    double* v = (double*)malloc((size_t)grid_size * sizeof(double));
    double* u_new = (double*)malloc((size_t)grid_size * sizeof(double));

    // Initial conditions: u[i] = cosh(x) - 1.0, v[i] = 0.0
    for (int i = 0; i < grid_size; i++) {
        double x = -2.0 + (double)i * dx;
        u[i] = cosh(x) - 1.0;
        v[i] = 0.0;
    }

    // Time evolution
    for (int t = 0; t < timesteps; t++) {
        double time_val = (double)t * dt;

        // Copy boundaries
        u_new[0] = cosh(-2.0) - 1.0;
        u_new[grid_size - 1] = cosh(2.0) - 1.0;

        // Interior points
        for (int j = 1; j < grid_size - 1; j++) {
            double x = -2.0 + (double)j * dx;

            double catenary_force = cosh(x) - u[j];

            double tension;
            if (fabs(x) > 0.0001) {
                tension = sinh(x) / x;
            } else {
                tension = 1.0;
            }

            double damping = -0.1 * tanh(v[j]);
            double thermal = exp(-0.01 * time_val);
            double laplacian = (u[j + 1] - 2.0 * u[j] + u[j - 1]) / (dx * dx);

            double accel = tension * laplacian + catenary_force * thermal + damping;
            v[j] += accel * dt;
            u_new[j] = u[j] + v[j] * dt;
        }

        // Copy u_new -> u
        for (int i = 0; i < grid_size; i++) {
            u[i] = u_new[i];
        }
    }

    // Checksum: sum of u[i] + ln(fabs(u[i]) + 1.0)
    double checksum = 0.0;
    for (int i = 0; i < grid_size; i++) {
        checksum += u[i] + log(fabs(u[i]) + 1.0);
    }

    free(u);
    free(v);
    free(u_new);

    return checksum;
}

static double get_time_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000.0 + ts.tv_nsec / 1000000.0;
}

int main(void) {
    int grid_size = 10000;
    int timesteps = 500;

    printf("Hyperbolic PDE (1D Catenary Cable) Benchmark\n");
    printf("Grid size: %d, Timesteps: %d\n", grid_size, timesteps);

    printf("Warming up (3 runs at 100 timesteps)...\n");
    for (int w = 0; w < 3; w++) {
        (void)run_hyperbolic_pde(grid_size, 100);
    }

    printf("Running measured iterations...\n");
    int iterations = 5;
    double min_time = 1e18;
    double result_checksum = 0.0;

    for (int i = 0; i < iterations; i++) {
        double start = get_time_ms();
        double checksum = run_hyperbolic_pde(grid_size, timesteps);
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
