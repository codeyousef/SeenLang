// Great Circle (Haversine) Distance Benchmark
// N=2,000,000 pairs, earth_radius=6371.0
// Golden ratio spiral point generation
const std = @import("std");
const math = std.math;

const EARTH_RADIUS: f64 = 6371.0;
const PHI: f64 = 1.618033988749895;
const PI: f64 = 3.141592653589793;

fn haversine(lat1: f64, lon1: f64, lat2: f64, lon2: f64) f64 {
    const half_dlat = (lat2 - lat1) / 2.0;
    const half_dlon = (lon2 - lon1) / 2.0;
    const a = math.sin(half_dlat) * math.sin(half_dlat) +
        math.cos(lat1) * math.cos(lat2) * math.sin(half_dlon) * math.sin(half_dlon);
    return EARTH_RADIUS * 2.0 * math.asin(@sqrt(a));
}

fn runGreatCircle(n: usize) f64 {
    var total_distance: f64 = 0.0;
    const two_pi = 2.0 * PI;
    const n_f: f64 = @floatFromInt(n);

    for (0..n) |i| {
        const i_f: f64 = @floatFromInt(i);
        const t1 = i_f / n_f;
        const lat1 = math.asin(2.0 * t1 - 1.0);
        const lon1 = two_pi * PHI * i_f;

        const j = i + 1;
        const j_f: f64 = @floatFromInt(j);
        const t2 = j_f / n_f;
        const lat2 = math.asin(2.0 * t2 - 1.0);
        const lon2 = two_pi * PHI * j_f;

        total_distance += haversine(lat1, lon1, lat2, lon2);
    }

    return total_distance;
}

pub fn main() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();

    const n: usize = 2000000;

    try stdout.print("Great Circle (Haversine) Distance Benchmark\n", .{});
    try stdout.print("N: {d} pairs\n", .{n});

    // Warmup
    try stdout.print("Warming up (3 runs at n=40000)...\n", .{});
    for (0..3) |_| {
        _ = runGreatCircle(40000);
    }

    // Measured iterations
    try stdout.print("Running measured iterations...\n", .{});
    const iterations = 5;
    var min_time: f64 = 1e18;
    var result_checksum: f64 = 0.0;

    for (0..iterations) |_| {
        var timer = try std.time.Timer.start();
        const checksum = runGreatCircle(n);
        const elapsed_ns = timer.read();
        const elapsed_ms = @as(f64, @floatFromInt(elapsed_ns)) / 1_000_000.0;
        if (elapsed_ms < min_time) {
            min_time = elapsed_ms;
            result_checksum = checksum;
        }
    }

    try stdout.print("Checksum: {d:.6}\n", .{result_checksum});
    try stdout.print("Min time: {d:.6} ms\n", .{min_time});
}
