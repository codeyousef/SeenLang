// Euler Totient (Number Theory) Benchmark
// Computes Euler's totient function phi(n) for n = 1..N using brute-force GCD
// Pure integer math -- fair comparison (no hardware intrinsic advantage)
// N=10000
const std = @import("std");

fn gcd(a_init: i64, b_init: i64) i64 {
    var a = a_init;
    var b = b_init;
    while (b > 0) {
        const temp = b;
        b = @rem(a, b);
        a = temp;
    }
    return a;
}

fn eulerTotient(n: i64) i64 {
    var count: i64 = 0;
    var k: i64 = 1;
    while (k < n) : (k += 1) {
        if (gcd(k, n) == 1) {
            count += 1;
        }
    }
    return count;
}

fn runTotientSum(limit: i64) i64 {
    var total: i64 = 0;
    var n: i64 = 1;
    while (n <= limit) : (n += 1) {
        total += eulerTotient(n);
    }
    return total;
}

pub fn main() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();

    const n: i64 = 10000;

    try stdout.print("Euler Totient (Number Theory) Benchmark\n", .{});
    try stdout.print("Computing phi(1) to phi({d})\n", .{n});

    // Warmup
    const warmup_runs = 3;
    try stdout.print("Warming up ({d} runs at n=1000)...\n", .{warmup_runs});
    for (0..warmup_runs) |_| {
        _ = runTotientSum(1000);
    }

    // Measured iterations
    try stdout.print("Running measured iterations...\n", .{});
    const iterations = 5;
    var min_time: f64 = 1e18;
    var result: i64 = 0;

    for (0..iterations) |_| {
        var timer = try std.time.Timer.start();
        const checksum = runTotientSum(n);
        const elapsed_ns = timer.read();
        const elapsed_ms = @as(f64, @floatFromInt(elapsed_ns)) / 1_000_000.0;
        if (elapsed_ms < min_time) {
            min_time = elapsed_ms;
            result = checksum;
        }
    }

    try stdout.print("Checksum: {d}\n", .{result});
    try stdout.print("Min time: {d:.6} ms\n", .{min_time});
    const n_f: f64 = @floatFromInt(n);
    try stdout.print("Totients per second: {d:.6} million\n", .{n_f / (min_time / 1000.0) / 1000000.0});
}
