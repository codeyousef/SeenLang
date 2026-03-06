// Binary Trees Benchmark
// Same flattened algorithm as C/Seen: stores child check values, not pointers
// min_depth=4, max_depth=20
const std = @import("std");

const TreeNode = struct {
    item: i64,
    left_val: i64,
    right_val: i64,
    has_left: bool,
    has_right: bool,
};

fn treeNewLeaf(allocator: std.mem.Allocator, item: i64) !*TreeNode {
    const n = try allocator.create(TreeNode);
    n.* = .{
        .item = item,
        .left_val = 0,
        .right_val = 0,
        .has_left = false,
        .has_right = false,
    };
    return n;
}

fn treeWithChildren(allocator: std.mem.Allocator, item: i64, left_check: i64, right_check: i64) !*TreeNode {
    const n = try allocator.create(TreeNode);
    n.* = .{
        .item = item,
        .left_val = left_check,
        .right_val = right_check,
        .has_left = true,
        .has_right = true,
    };
    return n;
}

fn treeCheck(n: *const TreeNode) i64 {
    var result = n.item;
    if (n.has_left) result += n.left_val;
    if (n.has_right) result -= n.right_val;
    return result;
}

fn makeTree(allocator: std.mem.Allocator, depth: i64) !*TreeNode {
    if (depth == 0) {
        return try treeNewLeaf(allocator, 0);
    }
    const left = try makeTree(allocator, depth - 1);
    const right = try makeTree(allocator, depth - 1);
    const left_check = treeCheck(left);
    const right_check = treeCheck(right);
    allocator.destroy(left);
    allocator.destroy(right);
    return try treeWithChildren(allocator, 0, left_check, right_check);
}

fn runBinaryTrees(allocator: std.mem.Allocator, min_depth: i64, max_depth: i64) !i64 {
    const stretch_depth = max_depth + 1;
    const stretch_tree = try makeTree(allocator, stretch_depth);
    const stretch_check = treeCheck(stretch_tree);
    allocator.destroy(stretch_tree);

    const long_lived_tree = try makeTree(allocator, max_depth);
    var total_check: i64 = stretch_check;

    var depth: i64 = min_depth;
    while (depth <= max_depth) : (depth += 2) {
        const shift_amount: u6 = @intCast(max_depth - depth + min_depth);
        const iterations: i64 = @as(i64, 1) << shift_amount;
        var check: i64 = 0;
        var i: i64 = 0;
        while (i < iterations) : (i += 1) {
            const temp = try makeTree(allocator, depth);
            check += treeCheck(temp);
            allocator.destroy(temp);
        }
        total_check += check;
    }

    total_check += treeCheck(long_lived_tree);
    allocator.destroy(long_lived_tree);
    return total_check;
}

pub fn main() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();

    const allocator = std.heap.c_allocator;

    const min_depth: i64 = 4;
    const max_depth: i64 = 20;

    try stdout.print("Binary Trees Benchmark\n", .{});
    try stdout.print("Max depth: {d}\n", .{max_depth});

    // Warmup
    try stdout.print("Warming up (1 run at depth 16)...\n", .{});
    _ = try runBinaryTrees(allocator, min_depth, 16);

    // Measured iterations
    try stdout.print("Running measured iterations...\n", .{});
    const iterations = 3;
    var min_time: f64 = 1e18;
    var result_check: i64 = 0;

    for (0..iterations) |_| {
        var timer = try std.time.Timer.start();
        const check = try runBinaryTrees(allocator, min_depth, max_depth);
        const elapsed_ns = timer.read();
        const elapsed_ms = @as(f64, @floatFromInt(elapsed_ns)) / 1_000_000.0;
        if (elapsed_ms < min_time) {
            min_time = elapsed_ms;
            result_check = check;
        }
    }

    try stdout.print("Checksum: {d}\n", .{result_check});
    try stdout.print("Min time: {d:.6} ms\n", .{min_time});
}
