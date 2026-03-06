// Reverse Complement Benchmark
// 256-entry complement lookup table, LCG-generated DNA sequence
// N=5,000,000
const std = @import("std");

fn initComplementTable() [256]u8 {
    var table: [256]u8 = undefined;
    // Initialize all entries to identity
    for (0..256) |i| {
        table[i] = @intCast(i);
    }

    // Uppercase complements
    table['A'] = 'T';
    table['T'] = 'A';
    table['C'] = 'G';
    table['G'] = 'C';
    table['U'] = 'A';
    table['M'] = 'K';
    table['K'] = 'M';
    table['R'] = 'Y';
    table['Y'] = 'R';
    table['W'] = 'W';
    table['S'] = 'S';
    table['V'] = 'B';
    table['B'] = 'V';
    table['H'] = 'D';
    table['D'] = 'H';
    table['N'] = 'N';

    // Lowercase complements
    table['a'] = 't';
    table['t'] = 'a';
    table['c'] = 'g';
    table['g'] = 'c';
    table['u'] = 'a';
    table['m'] = 'k';
    table['k'] = 'm';
    table['r'] = 'y';
    table['y'] = 'r';
    table['w'] = 'w';
    table['s'] = 's';
    table['v'] = 'b';
    table['b'] = 'v';
    table['h'] = 'd';
    table['d'] = 'h';
    table['n'] = 'n';

    return table;
}

fn generateSequence(seq: []u8, n: usize) void {
    const bases = [4]u8{ 'A', 'C', 'G', 'T' };
    var seed: i64 = 42;
    for (0..n) |i| {
        seed = @rem(seed *% 1103515245 +% 12345, @as(i64, 2147483647));
        if (seed < 0) seed = -seed;
        const idx: usize = @intCast(@rem(seed, 4));
        seq[i] = bases[idx];
    }
}

fn reverseComplement(seq: []const u8, result: []u8, n: usize, table: *const [256]u8) void {
    for (0..n) |i| {
        result[i] = table[seq[n - 1 - i]];
    }
}

fn computeChecksum(result: []const u8, n: usize) i64 {
    var sum: i64 = 0;
    for (0..n) |i| {
        sum += @as(i64, @intCast(result[i]));
    }
    return sum;
}

fn runRevcomp(allocator: std.mem.Allocator, n: usize, table: *const [256]u8) !i64 {
    const seq = try allocator.alloc(u8, n);
    defer allocator.free(seq);
    const result = try allocator.alloc(u8, n);
    defer allocator.free(result);

    generateSequence(seq, n);
    reverseComplement(seq, result, n, table);

    return computeChecksum(result, n);
}

pub fn main() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    const allocator = std.heap.page_allocator;

    const n: usize = 5_000_000;

    try stdout.print("Reverse Complement Benchmark\n", .{});
    try stdout.print("N: {d}\n", .{n});

    const complement_table = initComplementTable();

    // Warmup
    try stdout.print("Warming up (3 runs)...\n", .{});
    for (0..3) |_| {
        _ = try runRevcomp(allocator, n, &complement_table);
    }

    // Measured iterations
    try stdout.print("Running measured iterations...\n", .{});
    const iterations = 5;
    var min_time: f64 = 1e18;
    var result_checksum: i64 = 0;

    for (0..iterations) |_| {
        var timer = try std.time.Timer.start();
        const checksum = try runRevcomp(allocator, n, &complement_table);
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
