// Hyperbolic PDE Solver Benchmark
// 1D catenary/cable equation with hyperbolic function evaluation
// grid=10000, steps=500, dx=4.0/grid, dt=0.0001
const std = @import("std");
const math = std.math;

fn runHyperbolicPde(allocator: std.mem.Allocator, grid_size: usize, timesteps: usize) !f64 {
    const grid_f: f64 = @floatFromInt(grid_size);
    const dx: f64 = 4.0 / grid_f;
    const dt: f64 = 0.0001;

    const u = try allocator.alloc(f64, grid_size);
    defer allocator.free(u);
    const v = try allocator.alloc(f64, grid_size);
    defer allocator.free(v);
    const u_new = try allocator.alloc(f64, grid_size);
    defer allocator.free(u_new);

    // Set initial condition: catenary centered in domain
    for (0..grid_size) |i| {
        const i_f: f64 = @floatFromInt(i);
        const x = -2.0 + i_f * dx;
        u[i] = math.cosh(x) - 1.0;
        v[i] = 0.0;
    }

    // Time-stepping loop
    for (0..timesteps) |t| {
        const t_f: f64 = @floatFromInt(t);
        const time = t_f * dt;

        // Interior points update
        var j: usize = 1;
        while (j < grid_size - 1) : (j += 1) {
            const j_f: f64 = @floatFromInt(j);
            const x = -2.0 + j_f * dx;

            // Catenary restoring force: cosh(x)
            const catenary_force = math.cosh(x) - u[j];

            // Tension term using sinh: sinh(x)/x gives tension profile
            var tension: f64 = 1.0;
            if (@abs(x) > 0.0001) {
                tension = math.sinh(x) / x;
            }

            // Damping using tanh (saturates at +-1)
            const damping = -0.1 * math.tanh(v[j]);

            // Thermal decay using exp
            const thermal = @exp(-0.01 * time);

            // Laplacian (second derivative)
            const laplacian = (u[j + 1] - 2.0 * u[j] + u[j - 1]) / (dx * dx);

            // Update velocity and position
            const accel = tension * laplacian + catenary_force * thermal + damping;
            v[j] = v[j] + accel * dt;
            u_new[j] = u[j] + v[j] * dt;
        }

        // Boundary conditions: fixed ends
        u_new[0] = math.cosh(@as(f64, -2.0)) - 1.0;
        u_new[grid_size - 1] = math.cosh(@as(f64, 2.0)) - 1.0;

        // Copy u_new -> u
        @memcpy(u, u_new);
    }

    // Compute checksum: sum of final grid values plus some ln evaluations
    var checksum: f64 = 0.0;
    for (0..grid_size) |m| {
        checksum += u[m];
        // Also exercise ln on positive values
        const abs_val = @abs(u[m]) + 1.0;
        checksum += @log(abs_val);
    }

    return checksum;
}

pub fn main() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    const allocator = std.heap.page_allocator;

    const grid_size: usize = 10000;
    const timesteps: usize = 500;

    try stdout.print("Hyperbolic PDE Solver Benchmark\n", .{});
    try stdout.print("Grid size: {d}, Timesteps: {d}\n", .{ grid_size, timesteps });

    // Warmup
    const warmup_runs = 3;
    try stdout.print("Warming up ({d} runs at 100 timesteps)...\n", .{warmup_runs});
    for (0..warmup_runs) |_| {
        _ = try runHyperbolicPde(allocator, grid_size, 100);
    }

    // Measured iterations
    try stdout.print("Running measured iterations...\n", .{});
    const iterations = 5;
    var min_time: f64 = 1e18;
    var result: f64 = 0.0;

    for (0..iterations) |_| {
        var timer = try std.time.Timer.start();
        const checksum = try runHyperbolicPde(allocator, grid_size, timesteps);
        const elapsed_ns = timer.read();
        const elapsed_ms = @as(f64, @floatFromInt(elapsed_ns)) / 1_000_000.0;
        if (elapsed_ms < min_time) {
            min_time = elapsed_ms;
            result = checksum;
        }
    }

    const grid_f: f64 = @floatFromInt(grid_size);
    const steps_f: f64 = @floatFromInt(timesteps);
    const grid_points_per_step = grid_f * steps_f;

    try stdout.print("Checksum: {d:.6}\n", .{result});
    try stdout.print("Min time: {d:.6} ms\n", .{min_time});
    try stdout.print("Grid-steps per second: {d:.6} million\n", .{grid_points_per_step / (min_time / 1000.0) / 1000000.0});
}
