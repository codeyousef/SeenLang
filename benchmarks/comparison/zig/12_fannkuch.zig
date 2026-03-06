// Fannkuch-Redux Benchmark
// Same algorithm as C/Seen: Heap's algorithm variant for permutation generation
// Uses i64 for perm arrays (matching Seen's Int type)
// N=12
const std = @import("std");

const FannkuchResult = struct {
    checksum: i64,
    max_flips: i64,
};

fn runFannkuch(allocator: std.mem.Allocator, n: usize) !FannkuchResult {
    const perm = try allocator.alloc(i64, n);
    defer allocator.free(perm);
    const perm1 = try allocator.alloc(i64, n);
    defer allocator.free(perm1);
    const count = try allocator.alloc(i64, n);
    defer allocator.free(count);

    for (0..n) |i| {
        perm1[i] = @intCast(i);
    }

    var max_flips: i64 = 0;
    var checksum: i64 = 0;
    var perm_count: i64 = 0;
    var r: usize = n;

    while (true) {
        while (r != 1) {
            count[r - 1] = @intCast(r);
            r -= 1;
        }

        for (0..n) |i| {
            perm[i] = perm1[i];
        }

        // Count flips
        var flips: i64 = 0;
        var k: usize = @intCast(perm[0]);
        while (k != 0) {
            var lo: usize = 0;
            var hi: usize = k;
            while (lo < hi) {
                const tmp = perm[lo];
                perm[lo] = perm[hi];
                perm[hi] = tmp;
                lo += 1;
                hi -= 1;
            }
            flips += 1;
            k = @intCast(perm[0]);
        }

        if (flips > max_flips) max_flips = flips;

        if (@rem(perm_count, 2) == 0) {
            checksum += flips;
        } else {
            checksum -= flips;
        }
        perm_count += 1;

        // Generate next permutation
        r = 1;
        while (true) {
            if (r >= n) {
                return FannkuchResult{
                    .checksum = checksum,
                    .max_flips = max_flips,
                };
            }

            const perm0 = perm1[0];
            for (0..r) |i| {
                perm1[i] = perm1[i + 1];
            }
            perm1[r] = perm0;

            count[r] -= 1;
            if (count[r] > 0) break;
            r += 1;
        }
    }
}

pub fn main() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    const allocator = std.heap.page_allocator;

    const n: usize = 12;

    try stdout.print("Fannkuch-Redux Benchmark\n", .{});
    try stdout.print("N: {d}\n", .{n});

    // Warmup
    try stdout.print("Warming up (1 run at n=10)...\n", .{});
    _ = try runFannkuch(allocator, 10);

    // Measured iterations
    try stdout.print("Running measured iterations...\n", .{});
    const iterations = 3;
    var min_time: f64 = 1e18;
    var result_checksum: i64 = 0;
    var result_max_flips: i64 = 0;

    for (0..iterations) |_| {
        var timer = try std.time.Timer.start();
        const result = try runFannkuch(allocator, n);
        const elapsed_ns = timer.read();
        const elapsed_ms = @as(f64, @floatFromInt(elapsed_ns)) / 1_000_000.0;
        if (elapsed_ms < min_time) {
            min_time = elapsed_ms;
            result_checksum = result.checksum;
            result_max_flips = result.max_flips;
        }
    }

    try stdout.print("Checksum: {d}\n", .{result_checksum});
    try stdout.print("Max flips: {d}\n", .{result_max_flips});
    try stdout.print("Min time: {d:.6} ms\n", .{min_time});
}
