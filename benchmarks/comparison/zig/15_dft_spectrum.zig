// Discrete Fourier Transform (Power Spectrum) Benchmark
// Computes DFT of synthetic signal, then power spectrum in dB
// N=8192, signal = 3 sines (50, 120, 300 Hz)
// O(N^2) naive DFT
const std = @import("std");
const math = std.math;

fn runDft(allocator: std.mem.Allocator, n: usize) !f64 {
    const two_pi: f64 = 6.283185307179586;
    const n_f: f64 = @floatFromInt(n);

    // Generate synthetic signal: sum of 3 sine waves
    const signal = try allocator.alloc(f64, n);
    defer allocator.free(signal);

    for (0..n) |i| {
        const i_f: f64 = @floatFromInt(i);
        const t = i_f / n_f;
        // Frequencies: 50 Hz, 120 Hz, 300 Hz (sampled at n Hz)
        signal[i] = 1.0 * math.sin(two_pi * 50.0 * t) +
            0.5 * math.sin(two_pi * 120.0 * t) +
            0.3 * math.sin(two_pi * 300.0 * t);
    }

    // Compute DFT: X[k] = sum_{n=0}^{N-1} x[n] * exp(-j*2pi*k*n/N)
    // Real part: sum x[n] * cos(2pi*k*n/N)
    // Imag part: -sum x[n] * sin(2pi*k*n/N)
    const half_n = n / 2;
    const dft_re = try allocator.alloc(f64, half_n);
    defer allocator.free(dft_re);
    const dft_im = try allocator.alloc(f64, half_n);
    defer allocator.free(dft_im);

    for (0..half_n) |k| {
        var re_sum: f64 = 0.0;
        var im_sum: f64 = 0.0;
        const k_f: f64 = @floatFromInt(k);
        for (0..n) |j| {
            const j_f: f64 = @floatFromInt(j);
            const angle = two_pi * k_f * j_f / n_f;
            re_sum += signal[j] * math.cos(angle);
            im_sum -= signal[j] * math.sin(angle);
        }
        dft_re[k] = re_sum;
        dft_im[k] = im_sum;
    }

    // Compute power spectrum in dB and phase
    var checksum: f64 = 0.0;
    for (0..half_n) |m| {
        const power = dft_re[m] * dft_re[m] + dft_im[m] * dft_im[m];
        var power_db: f64 = 0.0;
        if (power > 0.000000001) {
            power_db = 10.0 * @log10(power);
        }
        const phase = math.atan2(dft_im[m], dft_re[m]);
        checksum += power_db + phase;
    }

    return checksum;
}

pub fn main() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    const allocator = std.heap.page_allocator;

    const n: usize = 8192;

    try stdout.print("DFT Power Spectrum Benchmark\n", .{});
    try stdout.print("Signal length: {d}\n", .{n});

    // Warmup
    const warmup_runs = 3;
    try stdout.print("Warming up ({d} runs at n=512)...\n", .{warmup_runs});
    for (0..warmup_runs) |_| {
        _ = try runDft(allocator, 512);
    }

    // Measured iterations
    try stdout.print("Running measured iterations...\n", .{});
    const iterations = 5;
    var min_time: f64 = 1e18;
    var result: f64 = 0.0;

    for (0..iterations) |_| {
        var timer = try std.time.Timer.start();
        const checksum = try runDft(allocator, n);
        const elapsed_ns = timer.read();
        const elapsed_ms = @as(f64, @floatFromInt(elapsed_ns)) / 1_000_000.0;
        if (elapsed_ms < min_time) {
            min_time = elapsed_ms;
            result = checksum;
        }
    }

    const n_f: f64 = @floatFromInt(n);
    const trig_calls = n_f * n_f;

    try stdout.print("Checksum: {d:.6}\n", .{result});
    try stdout.print("Min time: {d:.6} ms\n", .{min_time});
    try stdout.print("Trig calls per second: {d:.6} million\n", .{trig_calls / (min_time / 1000.0) / 1000000.0});
}
