// JSON Serialization Benchmark
// Same algorithm as C/Seen: StringBuilder pattern with reuse, 1M objects
// Uses {d:.6} for floats to match C's %.6f (6 decimal places)
const std = @import("std");

const StringBuilder = struct {
    data: []u8,
    length: usize,
    capacity: usize,
    allocator: std.mem.Allocator,

    fn init(alloc: std.mem.Allocator) !StringBuilder {
        const cap: usize = 4096;
        const data = try alloc.alloc(u8, cap);
        return StringBuilder{
            .data = data,
            .length = 0,
            .capacity = cap,
            .allocator = alloc,
        };
    }

    fn deinit(self: *StringBuilder) void {
        self.allocator.free(self.data);
    }

    fn clear(self: *StringBuilder) void {
        self.length = 0;
    }

    fn ensure(self: *StringBuilder, extra: usize) !void {
        if (self.length + extra > self.capacity) {
            while (self.length + extra > self.capacity) self.capacity *= 2;
            const new_data = try self.allocator.alloc(u8, self.capacity);
            @memcpy(new_data[0..self.length], self.data[0..self.length]);
            self.allocator.free(self.data);
            self.data = new_data;
        }
    }

    fn appendSlice(self: *StringBuilder, s: []const u8) !void {
        try self.ensure(s.len);
        @memcpy(self.data[self.length .. self.length + s.len], s);
        self.length += s.len;
    }

    fn appendInt(self: *StringBuilder, n: i64) !void {
        var buf: [32]u8 = undefined;
        var pos: usize = 0;

        var val = n;
        if (val < 0) {
            try self.ensure(1);
            self.data[self.length] = '-';
            self.length += 1;
            val = -val;
        }
        if (val == 0) {
            try self.ensure(1);
            self.data[self.length] = '0';
            self.length += 1;
            return;
        }
        // Build digits in reverse
        while (val > 0) {
            buf[pos] = @intCast(@as(u64, @intCast(@rem(val, 10))) + '0');
            pos += 1;
            val = @divTrunc(val, 10);
        }
        try self.ensure(pos);
        // Reverse copy
        var i: usize = 0;
        while (i < pos) : (i += 1) {
            self.data[self.length + i] = buf[pos - 1 - i];
        }
        self.length += pos;
    }

    fn appendFloat(self: *StringBuilder, f: f64) !void {
        // Format with 6 decimal places to match C's %.6f
        var buf: [64]u8 = undefined;
        const result = std.fmt.bufPrint(&buf, "{d:.6}", .{f}) catch return;
        try self.appendSlice(result);
    }
};

fn serializeInto(sb: *StringBuilder, id: i64, value: f64, active: bool, tags_json: []const u8) !void {
    try sb.appendSlice("{\"id\":");
    try sb.appendInt(id);
    try sb.appendSlice(",\"name\":\"Object");
    try sb.appendInt(id);
    try sb.appendSlice("\",\"value\":");
    try sb.appendFloat(value);
    if (active) {
        try sb.appendSlice(",\"active\":true,\"tags\":[");
    } else {
        try sb.appendSlice(",\"active\":false,\"tags\":[");
    }
    try sb.appendSlice(tags_json);
    try sb.appendSlice("]}");
}

fn benchmarkJson(allocator: std.mem.Allocator, n: i64) !i64 {
    // Seen's string escaping drops \" at boundaries, producing this 32-char string
    const tags_json = "tag1\",\"tag2\",\"tag3\",\"tag4\",\"tag5";
    var sb = try StringBuilder.init(allocator);
    defer sb.deinit();
    var total_length: i64 = 0;

    var i: i64 = 0;
    while (i < n) : (i += 1) {
        sb.clear();
        const active_val = (@rem(i, 2) == 0);
        const value = @as(f64, @floatFromInt(i)) * 3.14159;
        try serializeInto(&sb, i, value, active_val, tags_json);
        total_length += @as(i64, @intCast(sb.length));
    }

    return total_length;
}

pub fn main() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    const allocator = std.heap.page_allocator;

    const n: i64 = 1000000;

    try stdout.print("JSON Serialization Benchmark\n", .{});
    try stdout.print("Objects to serialize: {d}\n", .{n});

    // Warmup
    const warmup_runs = 3;
    try stdout.print("Warming up ({d} runs)...\n", .{warmup_runs});
    for (0..warmup_runs) |_| {
        _ = try benchmarkJson(allocator, n / 10);
    }

    // Measured iterations
    try stdout.print("Running measured iterations...\n", .{});
    const iterations = 5;
    var min_time: f64 = 1e18;
    var result_length: i64 = 0;

    for (0..iterations) |_| {
        var timer = try std.time.Timer.start();
        const total_length = try benchmarkJson(allocator, n);
        const elapsed_ns = timer.read();
        const elapsed_ms = @as(f64, @floatFromInt(elapsed_ns)) / 1_000_000.0;
        if (elapsed_ms < min_time) {
            min_time = elapsed_ms;
            result_length = total_length;
        }
    }

    try stdout.print("Total JSON length: {d}\n", .{result_length});
    try stdout.print("Min time: {d:.6} ms\n", .{min_time});
    const n_f: f64 = @floatFromInt(n);
    try stdout.print("Objects per second: {d:.6} thousand\n", .{n_f / (min_time / 1000.0) / 1000.0});
    const rl_f: f64 = @floatFromInt(result_length);
    try stdout.print("Throughput: {d:.6} MB/s\n", .{rl_f / (min_time / 1000.0) / 1e6});
}
