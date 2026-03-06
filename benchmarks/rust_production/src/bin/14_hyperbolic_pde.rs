// Hyperbolic PDE Solver Benchmark
// Faithful port of benchmarks/production/14_hyperbolic_pde.seen
// Same algorithm: 1D catenary equation, 10000 grid points, 500 timesteps

use std::time::Instant;

fn run_hyperbolic_pde(grid_size: usize, timesteps: usize) -> f64 {
    let dx = 4.0 / grid_size as f64;
    let dt = 0.0001;

    let mut u = vec![0.0f64; grid_size];
    let mut v = vec![0.0f64; grid_size];
    let mut u_new = vec![0.0f64; grid_size];

    // Set initial condition: catenary centered in domain
    let mut i = 0;
    while i < grid_size {
        let x = -2.0 + i as f64 * dx;
        u[i] = x.cosh() - 1.0;
        v[i] = 0.0;
        i += 1;
    }

    // Time-stepping loop
    let mut t = 0;
    while t < timesteps {
        let time = t as f64 * dt;

        // Interior points update
        let mut j = 1;
        while j < grid_size - 1 {
            let x = -2.0 + j as f64 * dx;

            // Catenary restoring force
            let catenary_force = x.cosh() - u[j];

            // Tension term using sinh
            let mut tension = 1.0;
            if x.abs() > 0.0001 {
                tension = x.sinh() / x;
            }

            // Damping using tanh
            let damping = -0.1 * v[j].tanh();

            // Thermal decay using exp
            let thermal = (-0.01 * time).exp();

            // Laplacian
            let laplacian = (u[j + 1] - 2.0 * u[j] + u[j - 1]) / (dx * dx);

            // Update velocity and position
            let accel = tension * laplacian + catenary_force * thermal + damping;
            v[j] = v[j] + accel * dt;
            u_new[j] = u[j] + v[j] * dt;

            j += 1;
        }

        // Boundary conditions
        u_new[0] = (-2.0_f64).cosh() - 1.0;
        u_new[grid_size - 1] = (2.0_f64).cosh() - 1.0;

        // Copy u_new -> u
        let mut k = 0;
        while k < grid_size {
            u[k] = u_new[k];
            k += 1;
        }

        t += 1;
    }

    // Compute checksum
    let mut checksum = 0.0;
    let mut m = 0;
    while m < grid_size {
        checksum += u[m];
        let abs_val = u[m].abs() + 1.0;
        checksum += abs_val.ln();
        m += 1;
    }

    checksum
}

fn main() {
    let grid_size = 10000;
    let timesteps = 500;

    println!("Hyperbolic PDE Solver Benchmark");
    println!("Grid size: {}, Timesteps: {}", grid_size, timesteps);

    // Warmup
    let warmup_runs = 3;
    println!("Warming up ({} runs at 100 timesteps)...", warmup_runs);
    for _ in 0..warmup_runs {
        let _ = run_hyperbolic_pde(grid_size, 100);
    }

    println!("Running measured iterations...");
    let iterations = 5;
    let mut min_time = f64::MAX;
    let mut result = 0.0;

    for _ in 0..iterations {
        let start = Instant::now();
        let checksum = run_hyperbolic_pde(grid_size, timesteps);
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;

        if elapsed < min_time {
            min_time = elapsed;
            result = checksum;
        }
    }

    let grid_points_per_step = grid_size as f64 * timesteps as f64;
    println!("Checksum: {:.9}", result);
    println!("Min time: {:.9} ms", min_time);
    println!(
        "Grid-steps per second: {:.9} million",
        grid_points_per_step / (min_time / 1000.0) / 1_000_000.0
    );
}
