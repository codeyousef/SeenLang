// Sieve of Eratosthenes Benchmark
// Same algorithm as C/Seen: bool flag array, limit=10,000,000
const std = @import("std");

fn sieveOfEratosthenes(allocator: std.mem.Allocator, limit: usize) ![]i64 {
    const is_prime = try allocator.alloc(bool, limit + 1);
    defer allocator.free(is_prime);
    @memset(is_prime, true);
    is_prime[0] = false;
    is_prime[1] = false;

    var p: usize = 2;
    while (p * p <= limit) : (p += 1) {
        if (is_prime[p]) {
            var j: usize = p * p;
            while (j <= limit) : (j += p) {
                is_prime[j] = false;
            }
        }
    }

    // Count primes first
    var count: usize = 0;
    for (2..limit + 1) |idx| {
        if (is_prime[idx]) count += 1;
    }

    // Allocate and fill
    const primes = try allocator.alloc(i64, count);
    var pi: usize = 0;
    for (2..limit + 1) |idx| {
        if (is_prime[idx]) {
            primes[pi] = @intCast(idx);
            pi += 1;
        }
    }

    return primes;
}

fn computeChecksum(primes: []const i64) i64 {
    var sum: i64 = 0;
    for (primes) |p| {
        sum += p;
    }
    return sum;
}

pub fn main() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    const allocator = std.heap.page_allocator;

    const limit: usize = 10_000_000;

    try stdout.print("Sieve of Eratosthenes Benchmark\n", .{});
    try stdout.print("Finding primes up to: {d}\n", .{limit});

    // Warmup
    const warmup_runs = 3;
    try stdout.print("Warming up ({d} runs)...\n", .{warmup_runs});
    for (0..warmup_runs) |_| {
        const p = try sieveOfEratosthenes(allocator, limit);
        allocator.free(p);
    }

    // Measured iterations
    try stdout.print("Running measured iterations...\n", .{});
    const iterations = 3;
    var min_time: f64 = 1e18;
    var result_primes: ?[]i64 = null;

    for (0..iterations) |_| {
        var timer = try std.time.Timer.start();
        const primes = try sieveOfEratosthenes(allocator, limit);
        const elapsed_ns = timer.read();
        const elapsed_ms = @as(f64, @floatFromInt(elapsed_ns)) / 1_000_000.0;

        if (elapsed_ms < min_time) {
            min_time = elapsed_ms;
            if (result_primes) |rp| allocator.free(rp);
            result_primes = primes;
        } else {
            allocator.free(primes);
        }
    }

    const primes = result_primes.?;
    const prime_count = primes.len;
    const checksum = computeChecksum(primes);

    try stdout.print("Primes found: {d}\n", .{prime_count});
    try stdout.print("Checksum: {d}\n", .{checksum});
    try stdout.print("Min time: {d:.6} ms\n", .{min_time});

    allocator.free(primes);
}
