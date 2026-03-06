// Zig Arithmetic Microbenchmark Implementation
// Equivalent to Seen's arithmetic operations for fair comparison

const std = @import("std");
const time = std.time;
const ArrayList = std.ArrayList;

const BenchmarkResult = struct {
    name: []const u8,
    language: []const u8,
    execution_time_ns: i64,
    memory_peak_bytes: i64,
    operations_per_second: f64,
    success: bool,
    error_message: ?[]const u8,
};

const ArithmeticBenchmark = struct {
    iterations: u32,
    data_size: usize,
    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator, iterations: u32, data_size: usize) ArithmeticBenchmark {
        return .{
            .iterations = iterations,
            .data_size = data_size,
            .allocator = allocator,
        };
    }

    // 32-bit integer addition benchmark
    pub fn benchmarkI32Addition(self: *const ArithmeticBenchmark) !BenchmarkResult {
        const vec_a = try self.allocator.alloc(i32, self.data_size);
        defer self.allocator.free(vec_a);
        const vec_b = try self.allocator.alloc(i32, self.data_size);
        defer self.allocator.free(vec_b);
        const result_vec = try self.allocator.alloc(i32, self.data_size);
        defer self.allocator.free(result_vec);

        // Initialize test data
        for (vec_a, vec_b, result_vec, 0..) |*a, *b, *r, i| {
            a.* = @intCast(i);
            b.* = @intCast(i * 2);
            r.* = 0;
        }

        const start = time.nanoTimestamp();

        var iter: u32 = 0;
        while (iter < self.iterations) : (iter += 1) {
            for (vec_a, vec_b, result_vec) |a, b, *r| {
                r.* = a + b;
            }
        }

        const elapsed_ns_i128 = time.nanoTimestamp() - start;
        const elapsed_ns: i64 = @intCast(elapsed_ns_i128);
        const total_operations: i64 = @intCast(self.iterations * @as(u32, @intCast(self.data_size)));
        const ops_per_second = @as(f64, @floatFromInt(total_operations)) / (@as(f64, @floatFromInt(elapsed_ns)) / 1e9);

        return BenchmarkResult{
            .name = "i32_addition",
            .language = "zig",
            .execution_time_ns = elapsed_ns,
            .memory_peak_bytes = @intCast(self.data_size * 3 * @sizeOf(i32)),
            .operations_per_second = ops_per_second,
            .success = true,
            .error_message = null,
        };
    }

    // 32-bit integer multiplication benchmark
    pub fn benchmarkI32Multiplication(self: *const ArithmeticBenchmark) !BenchmarkResult {
        const vec_a = try self.allocator.alloc(i32, self.data_size);
        defer self.allocator.free(vec_a);
        const vec_b = try self.allocator.alloc(i32, self.data_size);
        defer self.allocator.free(vec_b);
        const result_vec = try self.allocator.alloc(i32, self.data_size);
        defer self.allocator.free(result_vec);

        for (vec_a, vec_b, result_vec, 0..) |*a, *b, *r, i| {
            a.* = @intCast((i % 1000) + 1);
            b.* = @intCast((i % 500) + 1);
            r.* = 0;
        }

        const start = time.nanoTimestamp();

        var iter: u32 = 0;
        while (iter < self.iterations) : (iter += 1) {
            for (vec_a, vec_b, result_vec) |a, b, *r| {
                r.* = a * b;
            }
        }

        const elapsed_ns_i128 = time.nanoTimestamp() - start;
        const elapsed_ns: i64 = @intCast(elapsed_ns_i128);
        const total_operations: i64 = @intCast(self.iterations * @as(u32, @intCast(self.data_size)));
        const ops_per_second = @as(f64, @floatFromInt(total_operations)) / (@as(f64, @floatFromInt(elapsed_ns)) / 1e9);

        return BenchmarkResult{
            .name = "i32_multiplication",
            .language = "zig",
            .execution_time_ns = elapsed_ns,
            .memory_peak_bytes = @intCast(self.data_size * 3 * @sizeOf(i32)),
            .operations_per_second = ops_per_second,
            .success = true,
            .error_message = null,
        };
    }

    // 64-bit floating-point operations benchmark
    pub fn benchmarkF64Operations(self: *const ArithmeticBenchmark) !BenchmarkResult {
        const vec_a = try self.allocator.alloc(f64, self.data_size);
        defer self.allocator.free(vec_a);
        const vec_b = try self.allocator.alloc(f64, self.data_size);
        defer self.allocator.free(vec_b);
        const result_vec = try self.allocator.alloc(f64, self.data_size);
        defer self.allocator.free(result_vec);

        for (vec_a, vec_b, result_vec, 0..) |*a, *b, *r, i| {
            const fi: f64 = @floatFromInt(i);
            a.* = fi * 0.001 + 0.001;
            b.* = fi * 0.002 + 0.002;
            r.* = 0.0;
        }

        const start = time.nanoTimestamp();

        var iter: u32 = 0;
        while (iter < self.iterations) : (iter += 1) {
            for (vec_a, vec_b, result_vec) |a, b, *r| {
                const intermediate = a + b;
                const intermediate2 = intermediate * a;
                r.* = intermediate2 / b;
            }
        }

        const elapsed_ns_i128 = time.nanoTimestamp() - start;
        const elapsed_ns: i64 = @intCast(elapsed_ns_i128);
        const total_operations: i64 = @intCast(self.iterations * @as(u32, @intCast(self.data_size)) * 3);
        const ops_per_second = @as(f64, @floatFromInt(total_operations)) / (@as(f64, @floatFromInt(elapsed_ns)) / 1e9);

        return BenchmarkResult{
            .name = "f64_mixed_operations",
            .language = "zig",
            .execution_time_ns = elapsed_ns,
            .memory_peak_bytes = @intCast(self.data_size * 3 * @sizeOf(f64)),
            .operations_per_second = ops_per_second,
            .success = true,
            .error_message = null,
        };
    }

    // Bitwise operations benchmark
    pub fn benchmarkBitwiseOperations(self: *const ArithmeticBenchmark) !BenchmarkResult {
        const vec_a = try self.allocator.alloc(u32, self.data_size);
        defer self.allocator.free(vec_a);
        const vec_b = try self.allocator.alloc(u32, self.data_size);
        defer self.allocator.free(vec_b);
        const result_vec = try self.allocator.alloc(u32, self.data_size);
        defer self.allocator.free(result_vec);

        for (vec_a, vec_b, result_vec, 0..) |*a, *b, *r, i| {
            a.* = @intCast(i);
            b.* = @as(u32, @intCast(i)) *% 0x9E3779B9;
            r.* = 0;
        }

        const start = time.nanoTimestamp();

        var iter: u32 = 0;
        while (iter < self.iterations) : (iter += 1) {
            for (vec_a, vec_b, result_vec) |a, b, *r| {
                const and_result = a & b;
                const or_result = and_result | a;
                const xor_result = or_result ^ b;
                r.* = xor_result;
            }
        }

        const elapsed_ns_i128 = time.nanoTimestamp() - start;
        const elapsed_ns: i64 = @intCast(elapsed_ns_i128);
        const total_operations: i64 = @intCast(self.iterations * @as(u32, @intCast(self.data_size)) * 3);
        const ops_per_second = @as(f64, @floatFromInt(total_operations)) / (@as(f64, @floatFromInt(elapsed_ns)) / 1e9);

        return BenchmarkResult{
            .name = "bitwise_operations",
            .language = "zig",
            .execution_time_ns = elapsed_ns,
            .memory_peak_bytes = @intCast(self.data_size * 3 * @sizeOf(u32)),
            .operations_per_second = ops_per_second,
            .success = true,
            .error_message = null,
        };
    }

    pub fn runAll(self: *const ArithmeticBenchmark) !ArrayList(BenchmarkResult) {
        var results = ArrayList(BenchmarkResult).init(self.allocator);
        try results.append(try self.benchmarkI32Addition());
        try results.append(try self.benchmarkI32Multiplication());
        try results.append(try self.benchmarkF64Operations());
        try results.append(try self.benchmarkBitwiseOperations());
        return results;
    }
};

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    const benchmark = ArithmeticBenchmark.init(allocator, 1000, 100000);
    const results = try benchmark.runAll();
    defer results.deinit();

    const stdout = std.io.getStdOut().writer();
    for (results.items) |result| {
        try stdout.print("{s}: {d:.2} ops/sec ({d:.2}ms)\n", .{
            result.name,
            result.operations_per_second,
            @as(f64, @floatFromInt(result.execution_time_ns)) / 1_000_000.0,
        });
    }
}