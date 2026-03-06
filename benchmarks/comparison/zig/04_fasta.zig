// FASTA DNA Sequence Generation Benchmark
// LCG RNG, ALU repeat, IUB/HomoSapiens random frequency tables
// N=5,000,000 nucleotides, initial seed=42
const std = @import("std");

const IM: i64 = 139968;
const IA: i64 = 3877;
const IC: i64 = 29573;

var g_seed: i64 = 42;

fn lcgRandom() f64 {
    g_seed = @rem(g_seed * IA + IC, IM);
    return @as(f64, @floatFromInt(g_seed)) / @as(f64, @floatFromInt(IM));
}

const ALU = "GGCCGGGCGCGGTGGCTCACGCCTGTAATCCCAGCACTTTGGGAGGCCGAGGCGGGCGGATCACCTGAGGTCAGGAGTTCGAGACCAGCCTGGCCAAC" ++
    "ATGGTGAAACCCCGTCTCTACTAAAAATACAAAAATTAGCCGGGCGTGGTGGCGCGCGCCTGTAATCCCAGCTACTCGGGAGGCTGAGGCAGGAGAAT" ++
    "CGCTTGAACCCGGGAGGCGGAGGTTGCAGTGAGCCGAGATCGCGCCACTGCACTCCAGCCTGGGCGACAGAGCGAGACTCCGTCTCAAAAA";

const ALU_LEN: usize = 287;

const AminoAcid = struct {
    c: u8,
    p: f64,
};

// Original (non-cumulative) probabilities
const iub_orig = [15]AminoAcid{
    .{ .c = 'a', .p = 0.27 }, .{ .c = 'c', .p = 0.12 }, .{ .c = 'g', .p = 0.12 }, .{ .c = 't', .p = 0.27 },
    .{ .c = 'B', .p = 0.02 }, .{ .c = 'D', .p = 0.02 }, .{ .c = 'H', .p = 0.02 }, .{ .c = 'K', .p = 0.02 },
    .{ .c = 'M', .p = 0.02 }, .{ .c = 'N', .p = 0.02 }, .{ .c = 'R', .p = 0.02 }, .{ .c = 'S', .p = 0.02 },
    .{ .c = 'V', .p = 0.02 }, .{ .c = 'W', .p = 0.02 }, .{ .c = 'Y', .p = 0.02 },
};

const hs_orig = [4]AminoAcid{
    .{ .c = 'a', .p = 0.3029549426680 }, .{ .c = 'c', .p = 0.1979883004921 },
    .{ .c = 'g', .p = 0.1975473066391 }, .{ .c = 't', .p = 0.3015094502008 },
};

fn makeCumulative(table: []AminoAcid) void {
    var cp: f64 = 0.0;
    for (table) |*entry| {
        cp += entry.p;
        entry.p = cp;
    }
}

fn selectRandom(table: []const AminoAcid) u8 {
    const r = lcgRandom();
    for (table) |entry| {
        if (r < entry.p) return entry.c;
    }
    return table[table.len - 1].c;
}

fn repeatFasta(out: []u8, n: usize) i64 {
    var checksum: i64 = 0;
    for (0..n) |i| {
        const c = ALU[i % ALU_LEN];
        out[i] = c;
        checksum += @as(i64, @intCast(c));
    }
    return checksum;
}

fn randomFasta(out: []u8, n: usize, table: []const AminoAcid) i64 {
    var checksum: i64 = 0;
    for (0..n) |i| {
        const c = selectRandom(table);
        out[i] = c;
        checksum += @as(i64, @intCast(c));
    }
    return checksum;
}

fn runFasta(allocator: std.mem.Allocator, n: usize) !i64 {
    g_seed = 42;

    const repeat_n = n * 2;
    const iub_n = n * 3;
    const hs_n = n * 5;

    const buf1 = try allocator.alloc(u8, repeat_n);
    defer allocator.free(buf1);
    const buf2 = try allocator.alloc(u8, iub_n);
    defer allocator.free(buf2);
    const buf3 = try allocator.alloc(u8, hs_n);
    defer allocator.free(buf3);

    // Copy and make cumulative
    var iub: [15]AminoAcid = iub_orig;
    var hs: [4]AminoAcid = hs_orig;
    makeCumulative(&iub);
    makeCumulative(&hs);

    var checksum: i64 = 0;
    checksum += repeatFasta(buf1, repeat_n);
    checksum += randomFasta(buf2, iub_n, &iub);
    checksum += randomFasta(buf3, hs_n, &hs);

    return checksum;
}

pub fn main() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    const allocator = std.heap.page_allocator;

    const n: usize = 5_000_000;

    try stdout.print("FASTA DNA Sequence Generation Benchmark\n", .{});
    try stdout.print("N: {d} nucleotides\n", .{n});

    // Warmup
    try stdout.print("Warming up (3 runs)...\n", .{});
    for (0..3) |_| {
        _ = try runFasta(allocator, n);
    }

    // Measured iterations
    try stdout.print("Running measured iterations...\n", .{});
    const iterations = 5;
    var min_time: f64 = 1e18;
    var result_checksum: i64 = 0;

    for (0..iterations) |_| {
        var timer = try std.time.Timer.start();
        const checksum = try runFasta(allocator, n);
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
