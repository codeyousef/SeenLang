// Zig JSON parser benchmark implementation for fair comparison with Seen
// High-performance JSON parser using similar architecture to Seen implementation

const std = @import("std");
const print = std.debug.print;
const ArrayList = std.ArrayList;
const HashMap = std.HashMap;
const Allocator = std.mem.Allocator;

const JsonValue = union(enum) {
    null_value,
    bool_value: bool,
    number_value: f64,
    string_value: []const u8,
    array_value: ArrayList(JsonValue),
    object_value: HashMap([]const u8, JsonValue, std.hash_map.StringContext, std.hash_map.default_max_load_percentage),

    const Self = @This();

    pub fn isValid(self: Self) bool {
        return switch (self) {
            .null_value => true,
            .bool_value => true,
            .number_value => |n| std.math.isFinite(n),
            .string_value => |s| s.len > 0,
            .array_value => |arr| {
                for (arr.items) |item| {
                    if (!item.isValid()) return false;
                }
                return true;
            },
            .object_value => |obj| {
                var iterator = obj.iterator();
                while (iterator.next()) |entry| {
                    if (!entry.value_ptr.isValid()) return false;
                }
                return true;
            },
        };
    }

    pub fn size(self: Self) usize {
        return switch (self) {
            .null_value, .bool_value, .number_value => 1,
            .string_value => |s| s.len,
            .array_value => |arr| {
                var total: usize = 0;
                for (arr.items) |item| {
                    total += item.size();
                }
                return total;
            },
            .object_value => |obj| {
                var total: usize = obj.count(); // Count keys
                var iterator = obj.iterator();
                while (iterator.next()) |entry| {
                    total += entry.value_ptr.size();
                }
                return total;
            },
        };
    }

    pub fn deinit(self: *Self, allocator: Allocator) void {
        switch (self.*) {
            .string_value => |s| allocator.free(s),
            .array_value => |*arr| {
                for (arr.items) |*item| {
                    item.deinit(allocator);
                }
                arr.deinit();
            },
            .object_value => |*obj| {
                var iterator = obj.iterator();
                while (iterator.next()) |entry| {
                    allocator.free(entry.key_ptr.*);
                    entry.value_ptr.deinit(allocator);
                }
                obj.deinit();
            },
            else => {},
        }
    }
};

const JsonParser = struct {
    input: []const u8,
    position: usize,
    line: usize,
    column: usize,
    allocator: Allocator,

    const Self = @This();

    pub fn init(allocator: Allocator, input: []const u8) Self {
        return Self{
            .input = input,
            .position = 0,
            .line = 1,
            .column = 1,
            .allocator = allocator,
        };
    }

    fn isAtEnd(self: *const Self) bool {
        return self.position >= self.input.len;
    }

    fn currentChar(self: *const Self) u8 {
        if (self.isAtEnd()) return 0;
        return self.input[self.position];
    }

    fn peekChar(self: *const Self) u8 {
        if (self.position + 1 >= self.input.len) return 0;
        return self.input[self.position + 1];
    }

    fn advance(self: *Self) u8 {
        if (self.isAtEnd()) return 0;

        const ch = self.input[self.position];
        self.position += 1;

        if (ch == '\n') {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }

        return ch;
    }

    fn skipWhitespace(self: *Self) void {
        while (!self.isAtEnd()) {
            const ch = self.currentChar();
            if (ch == ' ' or ch == '\t' or ch == '\r' or ch == '\n') {
                _ = self.advance();
            } else {
                break;
            }
        }
    }

    fn parseUnicodeEscape(self: *Self) !u21 {
        var hex: [4]u8 = undefined;

        for (&hex) |*h| {
            if (self.isAtEnd() or !std.ascii.isHex(self.currentChar())) {
                return error.InvalidUnicodeEscape;
            }
            h.* = self.advance();
        }

        const hex_str = hex[0..4];
        const code_point = std.fmt.parseInt(u16, hex_str, 16) catch return error.InvalidUnicodeEscape;
        return @as(u21, code_point);
    }

    fn parseString(self: *Self) !JsonValue {
        _ = self.advance(); // consume opening quote

        var value = ArrayList(u8).init(self.allocator);
        defer value.deinit();

        while (!self.isAtEnd() and self.currentChar() != '"') {
            const ch = self.currentChar();

            if (ch == '\\') {
                _ = self.advance(); // consume backslash
                if (self.isAtEnd()) {
                    return error.UnexpectedEndOfInput;
                }

                const escaped = self.currentChar();
                switch (escaped) {
                    '"' => try value.append('"'),
                    '\\' => try value.append('\\'),
                    '/' => try value.append('/'),
                    'b' => try value.append('\u{0008}'),
                    'f' => try value.append('\u{000C}'),
                    'n' => try value.append('\n'),
                    'r' => try value.append('\r'),
                    't' => try value.append('\t'),
                    'u' => {
                        _ = self.advance(); // consume 'u'
                        const code_point = try self.parseUnicodeEscape();
                        var utf8_buf: [4]u8 = undefined;
                        const utf8_len = std.unicode.utf8Encode(code_point, &utf8_buf) catch return error.InvalidUnicode;
                        try value.appendSlice(utf8_buf[0..utf8_len]);
                        continue; // Don't advance again
                    },
                    else => return error.InvalidEscapeSequence,
                }
                _ = self.advance();
            } else {
                try value.append(ch);
                _ = self.advance();
            }
        }

        if (self.isAtEnd() or self.currentChar() != '"') {
            return error.UnterminatedString;
        }

        _ = self.advance(); // consume closing quote

        const owned_string = try self.allocator.dupe(u8, value.items);
        return JsonValue{ .string_value = owned_string };
    }

    fn parseNumber(self: *Self) !JsonValue {
        const start = self.position;

        // Handle negative sign
        if (self.currentChar() == '-') {
            _ = self.advance();
        }

        // Parse integer part
        if (self.currentChar() == '0') {
            _ = self.advance();
        } else if (std.ascii.isDigit(self.currentChar())) {
            while (!self.isAtEnd() and std.ascii.isDigit(self.currentChar())) {
                _ = self.advance();
            }
        } else {
            return error.InvalidNumber;
        }

        // Parse decimal part
        if (!self.isAtEnd() and self.currentChar() == '.') {
            _ = self.advance();

            if (self.isAtEnd() or !std.ascii.isDigit(self.currentChar())) {
                return error.InvalidNumber;
            }

            while (!self.isAtEnd() and std.ascii.isDigit(self.currentChar())) {
                _ = self.advance();
            }
        }

        // Parse exponent part
        if (!self.isAtEnd() and (self.currentChar() == 'e' or self.currentChar() == 'E')) {
            _ = self.advance();

            if (!self.isAtEnd() and (self.currentChar() == '+' or self.currentChar() == '-')) {
                _ = self.advance();
            }

            if (self.isAtEnd() or !std.ascii.isDigit(self.currentChar())) {
                return error.InvalidNumber;
            }

            while (!self.isAtEnd() and std.ascii.isDigit(self.currentChar())) {
                _ = self.advance();
            }
        }

        const number_str = self.input[start..self.position];
        const value = std.fmt.parseFloat(f64, number_str) catch return error.InvalidNumber;

        return JsonValue{ .number_value = value };
    }

    fn matchKeyword(self: *Self, keyword: []const u8) bool {
        if (self.position + keyword.len > self.input.len) return false;

        const slice = self.input[self.position..self.position + keyword.len];
        if (!std.mem.eql(u8, slice, keyword)) return false;

        // Check that the keyword is not part of a larger identifier
        const next_pos = self.position + keyword.len;
        if (next_pos < self.input.len and std.ascii.isAlphanumeric(self.input[next_pos])) {
            return false;
        }

        // Consume the keyword
        for (keyword) |_| {
            _ = self.advance();
        }

        return true;
    }

    fn parseBoolean(self: *Self) !JsonValue {
        if (self.matchKeyword("true")) {
            return JsonValue{ .bool_value = true };
        } else if (self.matchKeyword("false")) {
            return JsonValue{ .bool_value = false };
        } else {
            return error.InvalidBoolean;
        }
    }

    fn parseNull(self: *Self) !JsonValue {
        if (self.matchKeyword("null")) {
            return JsonValue{ .null_value = {} };
        } else {
            return error.InvalidNull;
        }
    }

    fn parseArray(self: *Self) !JsonValue {
        _ = self.advance(); // consume '['
        self.skipWhitespace();

        var elements = ArrayList(JsonValue).init(self.allocator);
        errdefer {
            for (elements.items) |*item| {
                item.deinit(self.allocator);
            }
            elements.deinit();
        }

        // Handle empty array
        if (!self.isAtEnd() and self.currentChar() == ']') {
            _ = self.advance();
            return JsonValue{ .array_value = elements };
        }

        while (true) {
            const value = try self.parseValue();
            try elements.append(value);

            self.skipWhitespace();

            if (self.isAtEnd()) {
                return error.UnexpectedEndOfInput;
            }

            const ch = self.currentChar();
            if (ch == ',') {
                _ = self.advance();
                self.skipWhitespace();
            } else if (ch == ']') {
                _ = self.advance();
                break;
            } else {
                return error.UnexpectedCharacter;
            }
        }

        return JsonValue{ .array_value = elements };
    }

    fn parseObject(self: *Self) !JsonValue {
        _ = self.advance(); // consume '{'
        self.skipWhitespace();

        var object = HashMap([]const u8, JsonValue, std.hash_map.StringContext, std.hash_map.default_max_load_percentage).init(self.allocator);
        errdefer {
            var iterator = object.iterator();
            while (iterator.next()) |entry| {
                self.allocator.free(entry.key_ptr.*);
                entry.value_ptr.deinit(self.allocator);
            }
            object.deinit();
        }

        // Handle empty object
        if (!self.isAtEnd() and self.currentChar() == '}') {
            _ = self.advance();
            return JsonValue{ .object_value = object };
        }

        while (true) {
            // Parse key (must be string)
            const key_value = try self.parseString();
            const key = switch (key_value) {
                .string_value => |s| s,
                else => return error.ObjectKeyMustBeString,
            };

            self.skipWhitespace();

            // Expect colon
            if (self.isAtEnd() or self.currentChar() != ':') {
                self.allocator.free(key);
                return error.ExpectedColon;
            }
            _ = self.advance();

            self.skipWhitespace();

            // Parse value
            const value = try self.parseValue();
            try object.put(key, value);

            self.skipWhitespace();

            if (self.isAtEnd()) {
                return error.UnexpectedEndOfInput;
            }

            const ch = self.currentChar();
            if (ch == ',') {
                _ = self.advance();
                self.skipWhitespace();
            } else if (ch == '}') {
                _ = self.advance();
                break;
            } else {
                return error.UnexpectedCharacter;
            }
        }

        return JsonValue{ .object_value = object };
    }

    fn parseValue(self: *Self) !JsonValue {
        self.skipWhitespace();

        if (self.isAtEnd()) {
            return error.UnexpectedEndOfInput;
        }

        const ch = self.currentChar();

        return switch (ch) {
            '"' => self.parseString(),
            '[' => self.parseArray(),
            '{' => self.parseObject(),
            't', 'f' => self.parseBoolean(),
            'n' => self.parseNull(),
            '-', '0'...'9' => self.parseNumber(),
            else => error.UnexpectedCharacter,
        };
    }

    pub fn parse(self: *Self) !JsonValue {
        const value = try self.parseValue();
        self.skipWhitespace();

        if (!self.isAtEnd()) {
            return error.UnexpectedContentAfterJson;
        }

        return value;
    }
};

// Utility functions
fn readFile(allocator: Allocator, filename: []const u8) ![]u8 {
    const file = std.fs.cwd().openFile(filename, .{}) catch |err| switch (err) {
        error.FileNotFound => {
            print("Error: File not found: {s}\n", .{filename});
            return err;
        },
        else => return err,
    };
    defer file.close();

    const file_size = try file.getEndPos();
    const contents = try allocator.alloc(u8, file_size);
    _ = try file.readAll(contents);

    return contents;
}

fn generateDeeplyNestedJson(allocator: Allocator, depth: usize) ![]u8 {
    var json = ArrayList(u8).init(allocator);
    defer json.deinit();

    for (0..depth) |_| {
        try json.appendSlice("{\"nested\":");
    }

    try json.appendSlice("\"value\"");

    for (0..depth) |_| {
        try json.append('}');
    }

    return try allocator.dupe(u8, json.items);
}

fn generateWideJson(allocator: Allocator, count: usize) ![]u8 {
    var json = ArrayList(u8).init(allocator);
    defer json.deinit();

    try json.append('{');

    for (0..count) |i| {
        if (i > 0) {
            try json.append(',');
        }
        try json.writer().print("\"key{}\":{}", .{ i, i });
    }

    try json.append('}');
    return try allocator.dupe(u8, json.items);
}

fn benchmarkJsonParserRealWorld(allocator: Allocator) !void {
    const test_files = [_][]const u8{
        "../../test_data/json_files/twitter.json",
        "../../test_data/json_files/canada.json",
        "../../test_data/json_files/citm_catalog.json",
        "../../test_data/json_files/large.json",
    };

    var total_elements: u64 = 0;
    var total_bytes: u64 = 0;
    var total_time: f64 = 0.0;

    for (test_files) |file_path| {
        // Check if file exists
        const file_exists = std.fs.cwd().access(file_path, .{}) catch |err| switch (err) {
            error.FileNotFound => false,
            else => return err,
        };

        if (!file_exists) {
            print("Warning: Test file {s} not found, skipping...\n", .{file_path});
            continue;
        }

        const content = readFile(allocator, file_path) catch |err| {
            print("Error reading {s}: {}\n", .{ file_path, err });
            continue;
        };
        defer allocator.free(content);

        const file_size = content.len;
        print("Testing Zig JSON parser performance on {s} ({} bytes)\n", .{ file_path, file_size });

        // Run multiple iterations for statistical accuracy
        const iterations = 10;
        var file_elements: usize = 0;
        var file_time: f64 = 0.0;

        var i: u32 = 0;
        while (i < iterations) : (i += 1) {
            var parser = JsonParser.init(allocator, content);

            const start_time = std.time.nanoTimestamp();
            var result = parser.parse() catch |err| {
                print("❌ JSON parsing failed: {}\n", .{err});
                return err;
            };
            const end_time = std.time.nanoTimestamp();

            if (!result.isValid()) {
                print("❌ Invalid JSON result\n");
                result.deinit(allocator);
                return error.InvalidJson;
            }

            file_elements = result.size();
            const elapsed = @as(f64, @floatFromInt(end_time - start_time)) / 1_000_000_000.0; // Convert to seconds
            file_time += elapsed;

            result.deinit(allocator);
        }

        const avg_time = file_time / @as(f64, @floatFromInt(iterations));
        const bytes_per_second = if (avg_time > 0.0) @as(f64, @floatFromInt(file_size)) / avg_time else 0.0;

        print("  Elements: {}, Avg Time: {d:.6}s, Bytes/sec: {d:.0}\n", .{ file_elements, avg_time, bytes_per_second });

        total_elements += file_elements;
        total_bytes += file_size;
        total_time += avg_time;
    }

    const overall_bytes_per_sec = if (total_time > 0.0) @as(f64, @floatFromInt(total_bytes)) / total_time else 0.0;

    print("\nZig JSON Parser Overall Performance:\n");
    print("  Total elements: {}\n", .{total_elements});
    print("  Total bytes: {}\n", .{total_bytes});
    print("  Total time: {d:.6}s\n", .{total_time});
    print("  Average bytes/second: {d:.0}\n", .{overall_bytes_per_sec});
    print("  Average MB/sec: {d:.2}\n", .{overall_bytes_per_sec / (1024.0 * 1024.0)});
}

fn benchmarkJsonParserStressTest(allocator: Allocator) !void {
    print("Running Zig JSON parser stress tests...\n");

    // Test deeply nested structures
    const deeply_nested = try generateDeeplyNestedJson(allocator, 1000);
    defer allocator.free(deeply_nested);

    var parser1 = JsonParser.init(allocator, deeply_nested);

    const start = std.time.nanoTimestamp();
    var result1 = try parser1.parse();
    const elapsed = std.time.nanoTimestamp() - start;

    if (!result1.isValid()) {
        result1.deinit(allocator);
        return error.InvalidJson;
    }

    print("  Deeply nested (1000 levels): {}μs\n", .{@divTrunc(elapsed, 1000)});
    result1.deinit(allocator);

    // Test wide structures
    const wide_structure = try generateWideJson(allocator, 10000);
    defer allocator.free(wide_structure);

    var parser2 = JsonParser.init(allocator, wide_structure);

    const start2 = std.time.nanoTimestamp();
    var result2 = try parser2.parse();
    const elapsed2 = std.time.nanoTimestamp() - start2;

    if (!result2.isValid()) {
        result2.deinit(allocator);
        return error.InvalidJson;
    }

    print("  Wide structure (10000 keys): {}μs\n", .{@divTrunc(elapsed2, 1000)});
    result2.deinit(allocator);
}

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    print("Running Zig JSON Parser Benchmarks...\n");

    benchmarkJsonParserRealWorld(allocator) catch |err| {
        print("Error running real-world JSON parser benchmark: {}\n", .{err});
        std.process.exit(1);
    };

    benchmarkJsonParserStressTest(allocator) catch |err| {
        print("Error running JSON parser stress test: {}\n", .{err});
        std.process.exit(1);
    };

    print("Zig JSON parser benchmarks completed successfully!\n");
}