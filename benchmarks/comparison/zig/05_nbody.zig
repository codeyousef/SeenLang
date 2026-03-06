// N-Body Simulation Benchmark
// Same algorithm as C/Seen: parallel arrays for body properties, 50M steps
const std = @import("std");

const N_BODIES: usize = 5;
const PI: f64 = 3.141592653589793;

fn advance(bx: *[N_BODIES]f64, by: *[N_BODIES]f64, bz: *[N_BODIES]f64, bvx: *[N_BODIES]f64, bvy: *[N_BODIES]f64, bvz: *[N_BODIES]f64, bmass: *const [N_BODIES]f64, dt: f64) void {
    for (0..N_BODIES) |i| {
        for (i + 1..N_BODIES) |j| {
            const dx = bx[i] - bx[j];
            const dy = by[i] - by[j];
            const dz = bz[i] - bz[j];
            const dist_sq = dx * dx + dy * dy + dz * dz;
            const dist = @sqrt(dist_sq);
            const mag = dt / (dist_sq * dist);
            const mj_mag = bmass[j] * mag;
            const mi_mag = bmass[i] * mag;
            bvx[i] -= dx * mj_mag;
            bvy[i] -= dy * mj_mag;
            bvz[i] -= dz * mj_mag;
            bvx[j] += dx * mi_mag;
            bvy[j] += dy * mi_mag;
            bvz[j] += dz * mi_mag;
        }
    }
    for (0..N_BODIES) |k| {
        bx[k] += dt * bvx[k];
        by[k] += dt * bvy[k];
        bz[k] += dt * bvz[k];
    }
}

fn energy(bx: *const [N_BODIES]f64, by: *const [N_BODIES]f64, bz: *const [N_BODIES]f64, bvx: *const [N_BODIES]f64, bvy: *const [N_BODIES]f64, bvz: *const [N_BODIES]f64, bmass: *const [N_BODIES]f64) f64 {
    var e: f64 = 0.0;
    for (0..N_BODIES) |i| {
        e += 0.5 * bmass[i] * (bvx[i] * bvx[i] + bvy[i] * bvy[i] + bvz[i] * bvz[i]);
        for (i + 1..N_BODIES) |j| {
            const dx = bx[i] - bx[j];
            const dy = by[i] - by[j];
            const dz = bz[i] - bz[j];
            const distance = @sqrt(dx * dx + dy * dy + dz * dz);
            e -= (bmass[i] * bmass[j]) / distance;
        }
    }
    return e;
}

fn offsetMomentum(bvx: *[N_BODIES]f64, bvy: *[N_BODIES]f64, bvz: *[N_BODIES]f64, bmass: *const [N_BODIES]f64, solar_mass: f64) void {
    var px: f64 = 0.0;
    var py: f64 = 0.0;
    var pz: f64 = 0.0;
    for (0..N_BODIES) |i| {
        px += bvx[i] * bmass[i];
        py += bvy[i] * bmass[i];
        pz += bvz[i] * bmass[i];
    }
    bvx[0] = -px / solar_mass;
    bvy[0] = -py / solar_mass;
    bvz[0] = -pz / solar_mass;
}

pub fn main() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();

    const solar_mass = 4.0 * PI * PI;
    const days_per_year: f64 = 365.24;
    const dt: f64 = 0.01;

    try stdout.print("N-Body Simulation Benchmark\n", .{});

    var bx = [N_BODIES]f64{ 0.0, 4.84143144246472090e+00, 8.34336671824457987e+00, 1.28943695621391310e+01, 1.53796971148509165e+01 };
    var by = [N_BODIES]f64{ 0.0, -1.16032004402742839e+00, 4.12479856412430479e+00, -1.51111514016986312e+01, -2.59193146099879641e+01 };
    var bz = [N_BODIES]f64{ 0.0, -1.03622044471123109e-01, -4.03523417114321381e-01, -2.23307578892655734e-01, 1.79258772950371181e-01 };
    var bvx = [N_BODIES]f64{ 0.0, 1.66007664274403694e-03 * days_per_year, -2.76742510726862411e-03 * days_per_year, 2.96460137564761618e-03 * days_per_year, 2.68067772490389322e-03 * days_per_year };
    var bvy = [N_BODIES]f64{ 0.0, 7.69901118419740425e-03 * days_per_year, 4.99852801234917238e-03 * days_per_year, 2.37847173959480950e-03 * days_per_year, 1.62824170038242295e-03 * days_per_year };
    var bvz = [N_BODIES]f64{ 0.0, -6.90460016972063023e-05 * days_per_year, 2.30417297573763929e-05 * days_per_year, -2.96589568540237556e-05 * days_per_year, -9.51592254519715870e-05 * days_per_year };
    const bmass = [N_BODIES]f64{ solar_mass, 9.54791938424326609e-04 * solar_mass, 2.85885980666130812e-04 * solar_mass, 4.36624404335156298e-05 * solar_mass, 5.15138902046611451e-05 * solar_mass };

    offsetMomentum(&bvx, &bvy, &bvz, &bmass, solar_mass);

    const num_steps: i64 = 50_000_000;
    try stdout.print("Simulating {d} steps\n", .{num_steps});

    // Save initial state for repeated measured runs
    const init_bx = bx;
    const init_by = by;
    const init_bz = bz;
    const init_bvx = bvx;
    const init_bvy = bvy;
    const init_bvz = bvz;

    // Warmup (1 run at 50000 steps)
    try stdout.print("Warming up (1 run at 50000 steps)...\n", .{});
    for (0..50000) |_| {
        advance(&bx, &by, &bz, &bvx, &bvy, &bvz, &bmass, dt);
    }

    // Measured iterations
    try stdout.print("Running measured iterations...\n", .{});
    const measured_iterations = 3;
    var min_time: f64 = 1e18;
    var result_initial_energy: f64 = 0.0;
    var result_final_energy: f64 = 0.0;
    const steps: usize = @intCast(num_steps);

    for (0..measured_iterations) |_| {
        // Restore initial state
        bx = init_bx;
        by = init_by;
        bz = init_bz;
        bvx = init_bvx;
        bvy = init_bvy;
        bvz = init_bvz;
        offsetMomentum(&bvx, &bvy, &bvz, &bmass, solar_mass);

        const ie = energy(&bx, &by, &bz, &bvx, &bvy, &bvz, &bmass);

        var timer = try std.time.Timer.start();
        for (0..steps) |_| {
            advance(&bx, &by, &bz, &bvx, &bvy, &bvz, &bmass, dt);
        }
        const elapsed_ns = timer.read();
        const elapsed_ms = @as(f64, @floatFromInt(elapsed_ns)) / 1_000_000.0;

        const fe = energy(&bx, &by, &bz, &bvx, &bvy, &bvz, &bmass);

        if (elapsed_ms < min_time) {
            min_time = elapsed_ms;
            result_initial_energy = ie;
            result_final_energy = fe;
        }
    }

    try stdout.print("Initial energy: {e}\n", .{result_initial_energy});
    try stdout.print("Final energy: {e}\n", .{result_final_energy});
    try stdout.print("Min time: {d:.6} ms\n", .{min_time});
    try stdout.print("Steps per second: {d:.6} million\n", .{@as(f64, @floatFromInt(num_steps)) / (min_time / 1000.0) / 1e6});
}
