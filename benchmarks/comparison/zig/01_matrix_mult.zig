// Matrix Multiplication Benchmark (SGEMM)
// Same algorithm as C/Seen: cache-blocked matrix multiply
// size=1024, block_size=64, seeds 12345/67890
const std = @import("std");

fn matrixFillRandom(data: []f64, seed_init: i64) void {
    var current_seed: i64 = seed_init;
    for (0..data.len) |i| {
        current_seed = @rem(current_seed *% 1103515245 +% 12345, @as(i64, 2147483647));
        if (current_seed < 0) current_seed = -current_seed;
        const value_int = @rem(current_seed, @as(i64, 10000));
        data[i] = @as(f64, @floatFromInt(value_int)) / 10000.0;
    }
}

fn matrixMultiply(a: []const f64, b: []const f64, c: []f64, size: usize) void {
    const block_size: usize = 64;
    var ii: usize = 0;
    while (ii < size) : (ii += block_size) {
        var jj: usize = 0;
        while (jj < size) : (jj += block_size) {
            var kk: usize = 0;
            while (kk < size) : (kk += block_size) {
                const i_end = @min(ii + block_size, size);
                const j_end = @min(jj + block_size, size);
                const k_end = @min(kk + block_size, size);
                var i: usize = ii;
                while (i < i_end) : (i += 1) {
                    var j: usize = jj;
                    while (j < j_end) : (j += 1) {
                        var sum: f64 = c[i * size + j];
                        var k: usize = kk;
                        while (k < k_end) : (k += 1) {
                            sum += a[i * size + k] * b[k * size + j];
                        }
                        c[i * size + j] = sum;
                    }
                }
            }
        }
    }
}

fn computeChecksum(data: []const f64) f64 {
    var sum: f64 = 0.0;
    for (data) |v| {
        sum += v;
    }
    return sum;
}

pub fn main() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    const allocator = std.heap.page_allocator;

    const size: usize = 1024;
    const total: usize = size * size;

    try stdout.print("Matrix Multiplication Benchmark (SGEMM)\n", .{});
    try stdout.print("Matrix size: {d}x{d}\n", .{ size, size });

    const a = try allocator.alloc(f64, total);
    defer allocator.free(a);
    const b = try allocator.alloc(f64, total);
    defer allocator.free(b);
    const c = try allocator.alloc(f64, total);
    defer allocator.free(c);

    matrixFillRandom(a, 12345);
    matrixFillRandom(b, 67890);

    // Warmup
    const warmup_runs = 3;
    try stdout.print("Warming up ({d} runs)...\n", .{warmup_runs});
    for (0..warmup_runs) |_| {
        matrixMultiply(a, b, c, size);
    }

    // Measured iterations
    try stdout.print("Running measured iterations...\n", .{});
    const iterations = 5;
    var min_time: f64 = 1e18;

    for (0..iterations) |_| {
        const c_fresh = try allocator.alloc(f64, total);
        defer allocator.free(c_fresh);
        @memset(c_fresh, 0.0);

        var timer = try std.time.Timer.start();
        matrixMultiply(a, b, c_fresh, size);
        const elapsed_ns = timer.read();
        const elapsed_ms = @as(f64, @floatFromInt(elapsed_ns)) / 1_000_000.0;
        if (elapsed_ms < min_time) min_time = elapsed_ms;
    }

    const checksum = computeChecksum(c);
    const size_f: f64 = @floatFromInt(size);
    const gflops = (2.0 * size_f * size_f * size_f) / (min_time / 1000.0) / 1e9;

    try stdout.print("Checksum: {d:.6}\n", .{checksum});
    try stdout.print("Min time: {d:.6} ms\n", .{min_time});
    try stdout.print("Performance: {d:.6} GFLOPS\n", .{gflops});
}
