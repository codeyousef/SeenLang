// Mandelbrot Escape-Time Benchmark
// 1000x1000, max_iter=100, bailout=4.0
// Domain: x in [-2.5, 1.0], y in [-1.0, 1.0]
const std = @import("std");

fn mandelbrotPixel(cx: f64, cy: f64, max_iter: i32) i32 {
    var zr: f64 = 0.0;
    var zi: f64 = 0.0;
    var i: i32 = 0;
    while (i < max_iter) : (i += 1) {
        const zr2 = zr * zr;
        const zi2 = zi * zi;
        if (zr2 + zi2 > 4.0) return i;
        zi = 2.0 * zr * zi + cy;
        zr = zr2 - zi2 + cx;
    }
    return max_iter;
}

fn runMandelbrot(width: i32, height: i32, max_iter: i32) i64 {
    var checksum: i64 = 0;
    const x_min: f64 = -2.5;
    const x_max: f64 = 1.0;
    const y_min: f64 = -1.0;
    const y_max: f64 = 1.0;

    const w_f: f64 = @floatFromInt(width);
    const h_f: f64 = @floatFromInt(height);

    var py: i32 = 0;
    while (py < height) : (py += 1) {
        const cy = y_min + @as(f64, @floatFromInt(py)) / h_f * (y_max - y_min);
        var px: i32 = 0;
        while (px < width) : (px += 1) {
            const cx = x_min + @as(f64, @floatFromInt(px)) / w_f * (x_max - x_min);
            const iters = mandelbrotPixel(cx, cy, max_iter);
            checksum += @as(i64, @intCast(iters));
        }
    }

    return checksum;
}

pub fn main() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();

    const width: i32 = 1000;
    const height: i32 = 1000;
    const max_iter: i32 = 100;

    try stdout.print("Mandelbrot Escape-Time Benchmark\n", .{});
    try stdout.print("Size: {d}x{d}, max_iter: {d}\n", .{ width, height, max_iter });

    // Warmup
    try stdout.print("Warming up (3 runs at 250x250)...\n", .{});
    for (0..3) |_| {
        _ = runMandelbrot(250, 250, max_iter);
    }

    // Measured iterations
    try stdout.print("Running measured iterations...\n", .{});
    const iterations = 5;
    var min_time: f64 = 1e18;
    var result_checksum: i64 = 0;

    for (0..iterations) |_| {
        var timer = try std.time.Timer.start();
        const checksum = runMandelbrot(width, height, max_iter);
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
