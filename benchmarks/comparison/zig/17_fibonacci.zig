// Fibonacci Benchmark
// Compute sum of first N fibonacci numbers using iterative method
// Tests loop performance and integer arithmetic
// N = 80000000, expected checksum depends on mod arithmetic (i64 overflow wraps)
const std = @import("std");

fn benchmarkFib(n: i64) i64 {
    var a: i64 = 0;
    var b: i64 = 1;
    var sum: i64 = 0;
    var i: i64 = 0;
    while (i < n) : (i += 1) {
        sum +%= a;
        const c = a +% b;
        a = b;
        b = c;
    }
    return sum;
}

pub fn main() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    const n: i64 = 1000000000;

    try stdout.print("Fibonacci Benchmark\n", .{});
    try stdout.print("Computing sum of first {d} fibonacci numbers\n", .{n});

    const warmup_runs: usize = 3;
    try stdout.print("Warming up ({d} runs)...\n", .{warmup_runs});
    for (0..warmup_runs) |_| {
        std.mem.doNotOptimizeAway(benchmarkFib(@divTrunc(n, 10)));
    }

    try stdout.print("Running measured iterations...\n", .{});
    const iterations: usize = 5;
    var min_time: f64 = 1e18;
    var result: i64 = 0;

    for (0..iterations) |_| {
        const start = std.time.nanoTimestamp();
        const answer = benchmarkFib(n);
        std.mem.doNotOptimizeAway(answer);
        const end = std.time.nanoTimestamp();
        const elapsed = @as(f64, @floatFromInt(end - start)) / 1_000_000.0;
        if (elapsed < min_time) {
            min_time = elapsed;
            result = answer;
        }
    }

    try stdout.print("Checksum: {d}\n", .{result});
    try stdout.print("Min time: {d:.6} ms\n", .{min_time});
}
