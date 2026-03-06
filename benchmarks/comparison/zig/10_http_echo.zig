// HTTP Echo Request/Response String Building Benchmark
// N=5,000,000 requests, checksum = sum of response string lengths
// Same algorithm as C: builds response string with snprintf equivalent each iteration
const std = @import("std");

fn runHttpEcho(n: i64) i64 {
    const body = "{\"user\":\"test\",\"action\":\"ping\",\"timestamp\":1234567890}";
    var response: [512]u8 = undefined;
    var total_length: i64 = 0;

    var i: i64 = 0;
    while (i < n) : (i += 1) {
        // Build response string in buffer (matching C's snprintf approach)
        const result = std.fmt.bufPrint(&response, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nServer: Seen/1.0\r\n\r\n{{\"echo\":\"{s}\"}}", .{body}) catch unreachable;
        total_length += @as(i64, @intCast(result.len));
    }

    return total_length;
}

pub fn main() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();

    const n: i64 = 5000000;

    try stdout.print("HTTP Echo Request/Response Benchmark\n", .{});
    try stdout.print("N: {d} requests\n", .{n});

    // Warmup
    const warmup_n = @divTrunc(n, 10);
    try stdout.print("Warming up (3 runs at n={d})...\n", .{warmup_n});
    for (0..3) |_| {
        _ = runHttpEcho(warmup_n);
    }

    // Measured iterations
    try stdout.print("Running measured iterations...\n", .{});
    const iterations = 5;
    var min_time: f64 = 1e18;
    var result_checksum: i64 = 0;

    for (0..iterations) |_| {
        var timer = try std.time.Timer.start();
        const checksum = runHttpEcho(n);
        const elapsed_ns = timer.read();
        const elapsed_ms = @as(f64, @floatFromInt(elapsed_ns)) / 1_000_000.0;
        if (elapsed_ms < min_time) {
            min_time = elapsed_ms;
            result_checksum = checksum;
        }
    }

    try stdout.print("Checksum: {d}\n", .{result_checksum});
    try stdout.print("Min time: {d:.6} ms\n", .{min_time});
}
