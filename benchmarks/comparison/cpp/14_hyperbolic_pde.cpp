// Hyperbolic PDE Solver Benchmark
// Same algorithm as Seen: 1D catenary/cable equation
// grid=10000, steps=500, dx=4.0/grid, dt=0.0001
#include <cstdio>
#include <cstdint>
#include <cmath>
#include <vector>
#include <chrono>

static double run_hyperbolic_pde(int grid_size, int timesteps) {
    double dx = 4.0 / (double)grid_size;
    double dt = 0.0001;

    std::vector<double> u((size_t)grid_size, 0.0);
    std::vector<double> v((size_t)grid_size, 0.0);
    std::vector<double> u_new((size_t)grid_size, 0.0);

    // Set initial condition: catenary centered in domain
    for (int i = 0; i < grid_size; i++) {
        double x = -2.0 + (double)i * dx;
        u[(size_t)i] = std::cosh(x) - 1.0;
        v[(size_t)i] = 0.0;
    }

    // Time-stepping loop
    for (int t = 0; t < timesteps; t++) {
        double time = (double)t * dt;

        // Interior points update
        for (int j = 1; j < grid_size - 1; j++) {
            double x = -2.0 + (double)j * dx;

            // Catenary restoring force
            double catenary_force = std::cosh(x) - u[(size_t)j];

            // Tension term using sinh
            double tension = 1.0;
            if (std::fabs(x) > 0.0001) {
                tension = std::sinh(x) / x;
            }

            // Damping using tanh (saturates at +-1)
            double damping = -0.1 * std::tanh(v[(size_t)j]);

            // Thermal decay using exp
            double thermal = std::exp(-0.01 * time);

            // Laplacian (second derivative)
            double laplacian = (u[(size_t)(j + 1)] - 2.0 * u[(size_t)j] + u[(size_t)(j - 1)]) / (dx * dx);

            // Update velocity and position
            double accel = tension * laplacian + catenary_force * thermal + damping;
            v[(size_t)j] = v[(size_t)j] + accel * dt;
            u_new[(size_t)j] = u[(size_t)j] + v[(size_t)j] * dt;
        }

        // Boundary conditions: fixed ends
        u_new[0] = std::cosh(-2.0) - 1.0;
        u_new[(size_t)(grid_size - 1)] = std::cosh(2.0) - 1.0;

        // Copy u_new -> u
        for (int k = 0; k < grid_size; k++) {
            u[(size_t)k] = u_new[(size_t)k];
        }
    }

    // Compute checksum: sum of final grid values plus some log evaluations
    double checksum = 0.0;
    for (int m = 0; m < grid_size; m++) {
        checksum += u[(size_t)m];
        double abs_val = std::fabs(u[(size_t)m]) + 1.0;
        checksum += std::log(abs_val);
    }

    return checksum;
}

int main() {
    int grid_size = 10000;
    int timesteps = 500;

    printf("Hyperbolic PDE Solver Benchmark\n");
    printf("Grid size: %d, Timesteps: %d\n", grid_size, timesteps);

    int warmup_runs = 3;
    printf("Warming up (%d runs at 100 timesteps)...\n", warmup_runs);
    for (int w = 0; w < warmup_runs; w++) {
        (void)run_hyperbolic_pde(grid_size, 100);
    }

    printf("Running measured iterations...\n");
    int iterations = 5;
    double min_time = 1e18;
    double result = 0.0;

    for (int iter = 0; iter < iterations; iter++) {
        auto start = std::chrono::high_resolution_clock::now();
        double checksum = run_hyperbolic_pde(grid_size, timesteps);
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
