// Spectral Norm Benchmark
// Same algorithm as C/Seen: eigenvalue approximation, N=5500, 10 power iterations
const std = @import("std");

fn evalA(i: usize, j: usize) f64 {
    const ij = i + j;
    return 1.0 / @as(f64, @floatFromInt(ij * (ij + 1) / 2 + i + 1));
}

fn multiplyAv(v: []const f64, out: []f64, n: usize) void {
    for (0..n) |i| {
        var sum: f64 = 0.0;
        for (0..n) |j| {
            sum += evalA(i, j) * v[j];
        }
        out[i] = sum;
    }
}

fn multiplyAtv(v: []const f64, out: []f64, n: usize) void {
    for (0..n) |i| {
        var sum: f64 = 0.0;
        for (0..n) |j| {
            sum += evalA(j, i) * v[j];
        }
        out[i] = sum;
    }
}

fn multiplyAtAv(v: []const f64, out: []f64, tmp: []f64, n: usize) void {
    multiplyAv(v, tmp, n);
    multiplyAtv(tmp, out, n);
}

fn runSpectralNorm(allocator: std.mem.Allocator, n: usize) !f64 {
    const u = try allocator.alloc(f64, n);
    defer allocator.free(u);
    const v = try allocator.alloc(f64, n);
    defer allocator.free(v);
    const tmp = try allocator.alloc(f64, n);
    defer allocator.free(tmp);

    // Initialize u to all 1.0, v and tmp to 0.0
    @memset(u, 1.0);
    @memset(v, 0.0);
    @memset(tmp, 0.0);

    // 10 power iterations
    for (0..10) |_| {
        multiplyAtAv(u, v, tmp, n);
        multiplyAtAv(v, u, tmp, n);
    }

    var vbv: f64 = 0.0;
    var vv: f64 = 0.0;
    for (0..n) |i| {
        vbv += u[i] * v[i];
        vv += v[i] * v[i];
    }

    return @sqrt(vbv / vv);
}

pub fn main() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    const allocator = std.heap.page_allocator;

    const n: usize = 5500;

    try stdout.print("Spectral Norm Benchmark\n", .{});
    try stdout.print("N: {d}\n", .{n});

    // Warmup
    const warmup_runs = 3;
    try stdout.print("Warming up ({d} runs at n=500)...\n", .{warmup_runs});
    for (0..warmup_runs) |_| {
        _ = try runSpectralNorm(allocator, 500);
    }

    // Measured iterations
    try stdout.print("Running measured iterations...\n", .{});
    const iterations = 5;
    var min_time: f64 = 1e18;
    var result: f64 = 0.0;

    for (0..iterations) |_| {
        var timer = try std.time.Timer.start();
        const norm = try runSpectralNorm(allocator, n);
        const elapsed_ns = timer.read();
        const elapsed_ms = @as(f64, @floatFromInt(elapsed_ns)) / 1_000_000.0;
        if (elapsed_ms < min_time) {
            min_time = elapsed_ms;
            result = norm;
        }
    }

    const n_f: f64 = @floatFromInt(n);
    const flops = 40.0 * 2.0 * n_f * n_f;
    const mflops = flops / (min_time / 1000.0) / 1e6;

    try stdout.print("Spectral norm: {d:.9}\n", .{result});
    try stdout.print("Min time: {d:.6} ms\n", .{min_time});
    try stdout.print("Throughput: {d:.6} MFLOPS\n", .{mflops});
}
