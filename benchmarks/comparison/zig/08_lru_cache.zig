// LRU Cache Benchmark
// Same algorithm as C/Seen: open-addressing hash map + parallel-array doubly-linked list
// 10M ops, capacity 100K, 70% get / 30% put
const std = @import("std");

const HashMap = struct {
    ht_keys: []i64,
    ht_vals: []i64,
    ht_flags: []i8, // 0=empty, 1=occupied, 2=tombstone
    ht_cap: i64,
    ht_size: i64,
    ht_tombstones: i64,
    allocator: std.mem.Allocator,

    fn init(allocator: std.mem.Allocator, cap: i64) !HashMap {
        // Round up to power of 2
        var c: i64 = 1;
        while (c < cap * 2) c <<= 1;
        const cu: usize = @intCast(c);

        const keys = try allocator.alloc(i64, cu);
        @memset(keys, 0);
        const vals = try allocator.alloc(i64, cu);
        @memset(vals, 0);
        const flags = try allocator.alloc(i8, cu);
        @memset(flags, 0);

        return HashMap{
            .ht_keys = keys,
            .ht_vals = vals,
            .ht_flags = flags,
            .ht_cap = c,
            .ht_size = 0,
            .ht_tombstones = 0,
            .allocator = allocator,
        };
    }

    fn deinit(self: *HashMap) void {
        self.allocator.free(self.ht_keys);
        self.allocator.free(self.ht_vals);
        self.allocator.free(self.ht_flags);
    }

    fn hash(key: i64, mask: i64) i64 {
        var h: u64 = @bitCast(key);
        h ^= h >> 33;
        h *%= 0xff51afd7ed558ccd;
        h ^= h >> 33;
        h *%= 0xc4ceb9fe1a85ec53;
        h ^= h >> 33;
        return @intCast(h & @as(u64, @intCast(mask)));
    }

    fn get(self: *const HashMap, key: i64, out_val: *i64) bool {
        const mask = self.ht_cap - 1;
        var idx = hash(key, mask);
        while (true) {
            const iu: usize = @intCast(idx);
            if (self.ht_flags[iu] == 0) return false; // empty
            if (self.ht_flags[iu] == 1 and self.ht_keys[iu] == key) {
                out_val.* = self.ht_vals[iu];
                return true;
            }
            idx = (idx + 1) & mask;
        }
    }

    fn insert(self: *HashMap, key: i64, val: i64) !void {
        if ((self.ht_size + self.ht_tombstones) * 10 >= self.ht_cap * 7) try self.grow();
        const mask = self.ht_cap - 1;
        var idx = hash(key, mask);
        while (true) {
            const iu: usize = @intCast(idx);
            if (self.ht_flags[iu] == 0) {
                self.ht_keys[iu] = key;
                self.ht_vals[iu] = val;
                self.ht_flags[iu] = 1;
                self.ht_size += 1;
                return;
            }
            if (self.ht_flags[iu] == 2) {
                self.ht_keys[iu] = key;
                self.ht_vals[iu] = val;
                self.ht_flags[iu] = 1;
                self.ht_size += 1;
                self.ht_tombstones -= 1;
                return;
            }
            if (self.ht_flags[iu] == 1 and self.ht_keys[iu] == key) {
                self.ht_vals[iu] = val;
                return;
            }
            idx = (idx + 1) & mask;
        }
    }

    fn remove(self: *HashMap, key: i64) void {
        const mask = self.ht_cap - 1;
        var idx = hash(key, mask);
        while (true) {
            const iu: usize = @intCast(idx);
            if (self.ht_flags[iu] == 0) return;
            if (self.ht_flags[iu] == 1 and self.ht_keys[iu] == key) {
                self.ht_flags[iu] = 2; // tombstone
                self.ht_size -= 1;
                self.ht_tombstones += 1;
                return;
            }
            idx = (idx + 1) & mask;
        }
    }

    fn grow(self: *HashMap) std.mem.Allocator.Error!void {
        const old_cap = self.ht_cap;
        const old_cap_u: usize = @intCast(old_cap);
        const old_keys = self.ht_keys;
        const old_vals = self.ht_vals;
        const old_flags = self.ht_flags;

        const new_cap = old_cap * 2;
        const new_cap_u: usize = @intCast(new_cap);
        self.ht_cap = new_cap;
        self.ht_size = 0;
        self.ht_tombstones = 0;
        self.ht_keys = try self.allocator.alloc(i64, new_cap_u);
        @memset(self.ht_keys, 0);
        self.ht_vals = try self.allocator.alloc(i64, new_cap_u);
        @memset(self.ht_vals, 0);
        self.ht_flags = try self.allocator.alloc(i8, new_cap_u);
        @memset(self.ht_flags, 0);

        for (0..old_cap_u) |i| {
            if (old_flags[i] == 1) {
                try self.insert(old_keys[i], old_vals[i]);
            }
        }
        self.allocator.free(old_keys);
        self.allocator.free(old_vals);
        self.allocator.free(old_flags);
    }
};

fn moveToFront(nodeIdx: i64, prev: []i64, next: []i64, head: *i64, tail: *i64) void {
    if (nodeIdx == head.*) return;
    const ni: usize = @intCast(nodeIdx);
    const p = prev[ni];
    const nx = next[ni];
    if (p >= 0) {
        const pu: usize = @intCast(p);
        next[pu] = nx;
    }
    if (nx >= 0) {
        const nxu: usize = @intCast(nx);
        prev[nxu] = p;
    }
    if (nodeIdx == tail.*) tail.* = p;
    prev[ni] = -1;
    next[ni] = head.*;
    if (head.* >= 0) {
        const hu: usize = @intCast(head.*);
        prev[hu] = nodeIdx;
    }
    head.* = nodeIdx;
}

fn benchmarkLru(allocator: std.mem.Allocator, n: i64, capacity: i64) !i64 {
    var map = try HashMap.init(allocator, capacity);
    defer map.deinit();

    const cap_u: usize = @intCast(capacity);
    const keys = try allocator.alloc(i64, cap_u);
    defer allocator.free(keys);
    @memset(keys, 0);
    const values = try allocator.alloc(i64, cap_u);
    defer allocator.free(values);
    @memset(values, 0);
    const prev = try allocator.alloc(i64, cap_u);
    defer allocator.free(prev);
    const next = try allocator.alloc(i64, cap_u);
    defer allocator.free(next);
    const freeList = try allocator.alloc(i64, cap_u);
    defer allocator.free(freeList);

    for (0..cap_u) |i| {
        prev[i] = -1;
        next[i] = -1;
        freeList[i] = @intCast(i);
    }

    var freeCount: i64 = capacity;
    var head: i64 = -1;
    var tail: i64 = -1;
    var size: i64 = 0;
    _ = &size;
    var checksum: i64 = 0;
    var rng: i64 = 42;

    var i: i64 = 0;
    while (i < n) : (i += 1) {
        // LCG: same as C version
        const rng_u: u64 = @bitCast(rng);
        rng = @bitCast(rng_u *% 1103515245 +% 12345);
        const key: i64 = if (rng < 0) -rng else rng;
        const rem2 = @rem(i, @as(i64, 10));

        if (rem2 < 7) {
            // GET
            var nodeIdx: i64 = undefined;
            if (map.get(key, &nodeIdx)) {
                const niu: usize = @intCast(nodeIdx);
                checksum += values[niu];
                moveToFront(nodeIdx, prev, next, &head, &tail);
            }
        } else {
            // PUT
            var nodeIdx: i64 = undefined;
            if (map.get(key, &nodeIdx)) {
                const niu: usize = @intCast(nodeIdx);
                values[niu] = key * 2;
                moveToFront(nodeIdx, prev, next, &head, &tail);
            } else {
                var ni: i64 = undefined;
                if (freeCount > 0) {
                    freeCount -= 1;
                    const fcu: usize = @intCast(freeCount);
                    ni = freeList[fcu];
                    size += 1;
                } else {
                    ni = tail;
                    const niu: usize = @intCast(ni);
                    const oldKey = keys[niu];
                    map.remove(oldKey);
                    const p = prev[niu];
                    tail = p;
                    if (p >= 0) {
                        const pu: usize = @intCast(p);
                        next[pu] = -1;
                    } else {
                        head = -1;
                    }
                }
                const niu: usize = @intCast(ni);
                keys[niu] = key;
                values[niu] = key * 2;
                prev[niu] = -1;
                next[niu] = head;
                if (head >= 0) {
                    const hu: usize = @intCast(head);
                    prev[hu] = ni;
                }
                head = ni;
                if (tail < 0) tail = ni;
                try map.insert(key, ni);
            }
        }
    }

    return checksum;
}

pub fn main() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    const allocator = std.heap.page_allocator;

    const n: i64 = 10_000_000;
    const capacity: i64 = 100_000;

    try stdout.print("LRU Cache Benchmark\n", .{});
    try stdout.print("Operations: {d}\n", .{n});
    try stdout.print("Capacity: {d}\n", .{capacity});

    // Warmup
    const warmup_runs = 3;
    try stdout.print("Warming up ({d} runs at 100000)...\n", .{warmup_runs});
    for (0..warmup_runs) |_| {
        _ = try benchmarkLru(allocator, 100_000, capacity);
    }

    // Measured iterations
    try stdout.print("Running measured iterations...\n", .{});
    const iterations = 3;
    var min_time: f64 = 1e18;
    var result_checksum: i64 = 0;

    for (0..iterations) |_| {
        var timer = try std.time.Timer.start();
        const checksum = try benchmarkLru(allocator, n, capacity);
        const elapsed_ns = timer.read();
        const elapsed_ms = @as(f64, @floatFromInt(elapsed_ns)) / 1_000_000.0;
        if (elapsed_ms < min_time) {
            min_time = elapsed_ms;
            result_checksum = checksum;
        }
    }

    try stdout.print("Checksum: {d}\n", .{result_checksum});
    try stdout.print("Min time: {d:.6} ms\n", .{min_time});
    try stdout.print("Operations per second: {d:.6} million\n", .{@as(f64, @floatFromInt(n)) / (min_time / 1000.0) / 1e6});
}
