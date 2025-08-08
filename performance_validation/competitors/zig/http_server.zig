// Zig HTTP server benchmark implementation for fair comparison with Seen
// High-performance HTTP server using similar architecture to Seen implementation

const std = @import("std");
const print = std.debug.print;
const ArrayList = std.ArrayList;
const HashMap = std.HashMap;
const Allocator = std.mem.Allocator;
const Thread = std.Thread;
const Atomic = std.atomic.Atomic;

// HTTP Request representation
const HttpRequest = struct {
    method: []const u8,
    path: []const u8,
    version: []const u8,
    headers: HashMap([]const u8, []const u8, std.hash_map.StringContext, std.hash_map.default_max_load_percentage),
    body: []const u8,
    allocator: Allocator,

    const Self = @This();

    pub fn init(allocator: Allocator) Self {
        return Self{
            .method = "",
            .path = "",
            .version = "",
            .headers = HashMap([]const u8, []const u8, std.hash_map.StringContext, std.hash_map.default_max_load_percentage).init(allocator),
            .body = "",
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *Self) void {
        var iterator = self.headers.iterator();
        while (iterator.next()) |entry| {
            self.allocator.free(entry.key_ptr.*);
            self.allocator.free(entry.value_ptr.*);
        }
        self.headers.deinit();
        
        if (self.method.len > 0) self.allocator.free(self.method);
        if (self.path.len > 0) self.allocator.free(self.path);
        if (self.version.len > 0) self.allocator.free(self.version);
        if (self.body.len > 0) self.allocator.free(self.body);
    }

    pub fn parseFromBuffer(allocator: Allocator, buffer: []const u8) !Self {
        var request = Self.init(allocator);
        errdefer request.deinit();

        var lines = std.mem.split(u8, buffer, "\n");
        
        // Parse request line (GET /path HTTP/1.1)
        const request_line = lines.next() orelse return error.EmptyRequest;
        var parts = std.mem.split(u8, std.mem.trimRight(u8, request_line, "\r"), " ");
        
        const method = parts.next() orelse return error.InvalidRequestLine;
        const path = parts.next() orelse return error.InvalidRequestLine;
        const version = parts.next() orelse return error.InvalidRequestLine;
        
        request.method = try allocator.dupe(u8, method);
        request.path = try allocator.dupe(u8, path);
        request.version = try allocator.dupe(u8, std.mem.trimRight(u8, version, "\r"));

        // Parse headers
        while (lines.next()) |line| {
            const trimmed_line = std.mem.trimRight(u8, line, "\r");
            if (trimmed_line.len == 0) break; // Empty line indicates end of headers
            
            if (std.mem.indexOf(u8, trimmed_line, ":")) |colon_pos| {
                const key = std.mem.trim(u8, trimmed_line[0..colon_pos], " \t");
                const value = std.mem.trim(u8, trimmed_line[colon_pos + 1..], " \t");
                
                // Convert key to lowercase for case-insensitive comparison
                var lower_key = try allocator.alloc(u8, key.len);
                for (key, 0..) |char, i| {
                    lower_key[i] = std.ascii.toLower(char);
                }
                
                const owned_value = try allocator.dupe(u8, value);
                try request.headers.put(lower_key, owned_value);
            }
        }

        // Parse body (remaining content)
        var body_content = ArrayList(u8).init(allocator);
        defer body_content.deinit();
        
        while (lines.next()) |line| {
            if (body_content.items.len > 0) {
                try body_content.append('\n');
            }
            try body_content.appendSlice(std.mem.trimRight(u8, line, "\r"));
        }
        
        request.body = try allocator.dupe(u8, body_content.items);
        return request;
    }

    pub fn getHeader(self: *const Self, name: []const u8) ?[]const u8 {
        // Create lowercase version of name for lookup
        var arena = std.heap.ArenaAllocator.init(self.allocator);
        defer arena.deinit();
        const arena_allocator = arena.allocator();
        
        const lower_name = arena_allocator.alloc(u8, name.len) catch return null;
        for (name, 0..) |char, i| {
            lower_name[i] = std.ascii.toLower(char);
        }
        
        return self.headers.get(lower_name);
    }
};

// HTTP Response representation
const HttpResponse = struct {
    version: []const u8 = "HTTP/1.1",
    status_code: u16,
    status_text: []const u8,
    headers: HashMap([]const u8, []const u8, std.hash_map.StringContext, std.hash_map.default_max_load_percentage),
    body: []const u8,
    allocator: Allocator,

    const Self = @This();

    pub fn init(allocator: Allocator, status_code: u16, status_text: []const u8) Self {
        return Self{
            .status_code = status_code,
            .status_text = status_text,
            .headers = HashMap([]const u8, []const u8, std.hash_map.StringContext, std.hash_map.default_max_load_percentage).init(allocator),
            .body = "",
            .allocator = allocator,
        };
    }

    pub fn ok(allocator: Allocator, body: []const u8) !Self {
        var response = Self.init(allocator, 200, "OK");
        try response.setBody(body);
        return response;
    }

    pub fn notFound(allocator: Allocator) !Self {
        var response = Self.init(allocator, 404, "Not Found");
        try response.setBody("<html><body><h1>404 Not Found</h1></body></html>");
        try response.setHeader("Content-Type", "text/html");
        return response;
    }

    pub fn internalServerError(allocator: Allocator) !Self {
        var response = Self.init(allocator, 500, "Internal Server Error");
        try response.setBody("<html><body><h1>500 Internal Server Error</h1></body></html>");
        try response.setHeader("Content-Type", "text/html");
        return response;
    }

    pub fn badRequest(allocator: Allocator) !Self {
        var response = Self.init(allocator, 400, "Bad Request");
        try response.setBody("<html><body><h1>400 Bad Request</h1></body></html>");
        try response.setHeader("Content-Type", "text/html");
        return response;
    }

    pub fn deinit(self: *Self) void {
        var iterator = self.headers.iterator();
        while (iterator.next()) |entry| {
            self.allocator.free(entry.key_ptr.*);
            self.allocator.free(entry.value_ptr.*);
        }
        self.headers.deinit();
        
        if (self.body.len > 0) self.allocator.free(self.body);
    }

    pub fn setHeader(self: *Self, key: []const u8, value: []const u8) !void {
        const owned_key = try self.allocator.dupe(u8, key);
        const owned_value = try self.allocator.dupe(u8, value);
        try self.headers.put(owned_key, owned_value);
    }

    pub fn setBody(self: *Self, body: []const u8) !void {
        if (self.body.len > 0) {
            self.allocator.free(self.body);
        }
        self.body = try self.allocator.dupe(u8, body);
        
        const content_length = try std.fmt.allocPrint(self.allocator, "{}", .{body.len});
        defer self.allocator.free(content_length);
        try self.setHeader("Content-Length", content_length);
    }

    pub fn toBytes(self: *const Self, allocator: Allocator) ![]u8 {
        var response = ArrayList(u8).init(allocator);
        defer response.deinit();

        try response.writer().print("{s} {} {s}\r\n", .{ self.version, self.status_code, self.status_text });

        var iterator = self.headers.iterator();
        while (iterator.next()) |entry| {
            try response.writer().print("{s}: {s}\r\n", .{ entry.key_ptr.*, entry.value_ptr.* });
        }

        try response.appendSlice("\r\n");
        try response.appendSlice(self.body);

        return try allocator.dupe(u8, response.items);
    }
};

// Thread pool for handling concurrent connections
const ThreadPool = struct {
    threads: []Thread,
    tasks: std.fifo.LinearFifo(Task, .Dynamic),
    mutex: Thread.Mutex,
    condition: Thread.Condition,
    stop: Atomic(bool),
    allocator: Allocator,

    const Task = *const fn (socket: std.os.socket_t, server_data: *ServerData) void;

    const Self = @This();

    pub fn init(allocator: Allocator, num_threads: usize) !Self {
        var pool = Self{
            .threads = try allocator.alloc(Thread, num_threads),
            .tasks = std.fifo.LinearFifo(Task, .Dynamic).init(allocator),
            .mutex = Thread.Mutex{},
            .condition = Thread.Condition{},
            .stop = Atomic(bool).init(false),
            .allocator = allocator,
        };

        return pool;
    }

    pub fn start(self: *Self, server_data: *ServerData) !void {
        for (self.threads) |*thread| {
            thread.* = try Thread.spawn(.{}, workerThread, .{ self, server_data });
        }
    }

    pub fn deinit(self: *Self) void {
        self.stop.store(true, .Monotonic);
        self.condition.broadcast();
        
        for (self.threads) |thread| {
            thread.join();
        }
        
        self.tasks.deinit();
        self.allocator.free(self.threads);
    }

    pub fn execute(self: *Self, task: Task) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        
        self.tasks.writeItem(task) catch return; // Drop task if queue is full
        self.condition.signal();
    }

    fn workerThread(self: *Self, server_data: *ServerData) void {
        while (true) {
            self.mutex.lock();
            
            while (self.tasks.readableLength() == 0 and !self.stop.load(.Monotonic)) {
                self.condition.wait(&self.mutex);
            }
            
            if (self.stop.load(.Monotonic)) {
                self.mutex.unlock();
                break;
            }
            
            const task = self.tasks.readItem().?;
            self.mutex.unlock();
            
            // Execute task - this will be set by the server when adding tasks
        }
    }
};

// Server data shared across threads
const ServerData = struct {
    request_count: Atomic(u64),
    response_time_sum: Atomic(u64), // in microseconds
    allocator: Allocator,

    pub fn init(allocator: Allocator) Self {
        return Self{
            .request_count = Atomic(u64).init(0),
            .response_time_sum = Atomic(u64).init(0),
            .allocator = allocator,
        };
    }
};

// Simple HTTP Server implementation
const HttpServer = struct {
    socket: std.os.socket_t,
    port: u16,
    running: Atomic(bool),
    server_data: ServerData,
    allocator: Allocator,

    const Self = @This();

    pub fn init(allocator: Allocator, port: u16) !Self {
        const socket = try std.os.socket(std.os.AF.INET, std.os.SOCK.STREAM, 0);
        
        // Set SO_REUSEADDR
        const opt: i32 = 1;
        try std.os.setsockopt(socket, std.os.SOL.SOCKET, std.os.SO.REUSEADDR, std.mem.asBytes(&opt));

        const address = std.net.Address.initIp4([_]u8{ 127, 0, 0, 1 }, port);
        try std.os.bind(socket, &address.any, address.getOsSockLen());
        try std.os.listen(socket, 10);

        return Self{
            .socket = socket,
            .port = port,
            .running = Atomic(bool).init(false),
            .server_data = ServerData.init(allocator),
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *Self) void {
        self.stop();
        std.os.closeSocket(self.socket);
    }

    fn handleRequest(self: *Self, request: *const HttpRequest) !HttpResponse {
        const start_time = std.time.nanoTimestamp();

        const response = blk: {
            if (std.mem.eql(u8, request.path, "/")) {
                break :blk try HttpResponse.ok(self.allocator, "Hello, World!");
            } else if (std.mem.eql(u8, request.path, "/health")) {
                var response = try HttpResponse.ok(self.allocator, "OK");
                try response.setHeader("Content-Type", "text/plain");
                break :blk response;
            } else if (std.mem.eql(u8, request.path, "/stats")) {
                const request_count = self.server_data.request_count.load(.Monotonic);
                const avg_response_time = if (request_count > 0) 
                    self.server_data.response_time_sum.load(.Monotonic) / request_count 
                else 
                    0;

                const stats = try std.fmt.allocPrint(self.allocator, 
                    "{{\"requests\": {}, \"avgResponseTimeUs\": {}}}", 
                    .{ request_count, avg_response_time });
                defer self.allocator.free(stats);

                var response = try HttpResponse.ok(self.allocator, stats);
                try response.setHeader("Content-Type", "application/json");
                break :blk response;
            } else if (std.mem.eql(u8, request.path, "/echo")) {
                var response = try HttpResponse.ok(self.allocator, request.body);
                try response.setHeader("Content-Type", "text/plain");
                break :blk response;
            } else if (std.mem.startsWith(u8, request.path, "/static/")) {
                const file_name = request.path[8..]; // Remove "/static/" prefix
                break :blk try self.serveStaticFile(file_name);
            } else {
                break :blk try HttpResponse.notFound(self.allocator);
            }
        };

        const end_time = std.time.nanoTimestamp();
        const response_time = @intCast(u64, @divTrunc(end_time - start_time, 1000)); // Convert to microseconds

        _ = self.server_data.request_count.fetchAdd(1, .Monotonic);
        _ = self.server_data.response_time_sum.fetchAdd(response_time, .Monotonic);

        return response;
    }

    fn serveStaticFile(self: *Self, file_name: []const u8) !HttpResponse {
        // Security: basic path traversal protection
        if (std.mem.indexOf(u8, file_name, "..") != null or std.mem.indexOf(u8, file_name, "/") != null) {
            return try HttpResponse.notFound(self.allocator);
        }

        const file_path = try std.fmt.allocPrint(self.allocator, "static/{s}", .{file_name});
        defer self.allocator.free(file_path);

        const content = std.fs.cwd().readFileAlloc(self.allocator, file_path, std.math.maxInt(usize)) catch {
            return try HttpResponse.notFound(self.allocator);
        };
        defer self.allocator.free(content);

        var response = try HttpResponse.ok(self.allocator, content);

        // Set content type based on file extension
        if (std.mem.lastIndexOf(u8, file_name, ".")) |dot_pos| {
            const extension = file_name[dot_pos + 1..];
            const content_type = if (std.mem.eql(u8, extension, "html"))
                "text/html"
            else if (std.mem.eql(u8, extension, "css"))
                "text/css"
            else if (std.mem.eql(u8, extension, "js"))
                "application/javascript"
            else if (std.mem.eql(u8, extension, "json"))
                "application/json"
            else if (std.mem.eql(u8, extension, "txt"))
                "text/plain"
            else
                "application/octet-stream";

            try response.setHeader("Content-Type", content_type);
        }

        return response;
    }

    fn handleConnection(self: *Self, client_socket: std.os.socket_t) void {
        defer std.os.closeSocket(client_socket);

        var buffer: [4096]u8 = undefined;
        const bytes_read = std.os.recv(client_socket, &buffer, 0) catch return;

        if (bytes_read == 0) return; // Connection closed by client

        const request_data = buffer[0..bytes_read];

        var request = HttpRequest.parseFromBuffer(self.allocator, request_data) catch {
            // Send 400 Bad Request for malformed requests
            var response = HttpResponse.badRequest(self.allocator) catch return;
            defer response.deinit();
            
            const response_bytes = response.toBytes(self.allocator) catch return;
            defer self.allocator.free(response_bytes);
            
            _ = std.os.send(client_socket, response_bytes, 0) catch {};
            return;
        };
        defer request.deinit();

        var response = self.handleRequest(&request) catch {
            var error_response = HttpResponse.internalServerError(self.allocator) catch return;
            defer error_response.deinit();
            
            const response_bytes = error_response.toBytes(self.allocator) catch return;
            defer self.allocator.free(response_bytes);
            
            _ = std.os.send(client_socket, response_bytes, 0) catch {};
            return;
        };
        defer response.deinit();

        const response_bytes = response.toBytes(self.allocator) catch return;
        defer self.allocator.free(response_bytes);

        _ = std.os.send(client_socket, response_bytes, 0) catch {};
    }

    pub fn start(self: *Self) !void {
        self.running.store(true, .Monotonic);
        
        print("HTTP Server started on 127.0.0.1:{}\n", .{self.port});

        // Create worker threads
        var threads: [4]Thread = undefined;
        for (&threads) |*thread| {
            thread.* = try Thread.spawn(.{}, serverWorker, .{self});
        }

        // Main accept loop
        while (self.running.load(.Monotonic)) {
            const client_socket = std.os.accept(self.socket, null, null, 0) catch |err| switch (err) {
                error.WouldBlock => continue,
                else => {
                    if (self.running.load(.Monotonic)) {
                        std.debug.print("Error accepting connection: {}\n", .{err});
                    }
                    continue;
                },
            };

            // Handle connection in a separate thread (simplified approach)
            const handler_thread = Thread.spawn(.{}, connectionHandler, .{ self, client_socket }) catch |err| {
                std.debug.print("Failed to spawn handler thread: {}\n", .{err});
                std.os.closeSocket(client_socket);
                continue;
            };
            handler_thread.detach();
        }

        // Wait for worker threads to finish
        for (threads) |thread| {
            thread.join();
        }
    }

    fn serverWorker(self: *Self) void {
        while (self.running.load(.Monotonic)) {
            std.time.sleep(std.time.ns_per_ms); // Simple polling approach
        }
    }

    fn connectionHandler(self: *Self, client_socket: std.os.socket_t) void {
        self.handleConnection(client_socket);
    }

    pub fn stop(self: *Self) void {
        self.running.store(false, .Monotonic);
    }

    pub fn getStats(self: *const Self) struct { u64, f64 } {
        const request_count = self.server_data.request_count.load(.Monotonic);
        const avg_response_time = if (request_count > 0)
            @intToFloat(f64, self.server_data.response_time_sum.load(.Monotonic)) / @intToFloat(f64, request_count)
        else
            0.0;

        return .{ request_count, avg_response_time };
    }
};

// Benchmark functions
fn benchmarkHttpServerThroughput(allocator: Allocator) !void {
    print("Running Zig HTTP server throughput benchmark...\n");

    // Start server in background thread
    var server = try HttpServer.init(allocator, 8080);
    defer server.deinit();

    const server_thread = try Thread.spawn(.{}, serverRunner, .{&server});

    // Wait for server to start
    std.time.sleep(200 * std.time.ns_per_ms);

    // Run concurrent load test
    const start_time = std.time.nanoTimestamp();
    const number_of_clients = 50;
    const requests_per_client = 100;
    
    var client_threads = try allocator.alloc(Thread, number_of_clients);
    defer allocator.free(client_threads);

    for (client_threads, 0..) |*thread, client_id| {
        thread.* = try Thread.spawn(.{}, clientWorker, .{ allocator, client_id, requests_per_client });
    }

    // Wait for all clients to complete
    for (client_threads) |thread| {
        thread.join();
    }

    const end_time = std.time.nanoTimestamp();
    const elapsed = @intToFloat(f64, end_time - start_time) / @intToFloat(f64, std.time.ns_per_s);
    const total_requests = number_of_clients * requests_per_client;
    const requests_per_second = @intToFloat(f64, total_requests) / elapsed;

    // Stop server
    server.stop();

    // Connect once more to wake up the server
    const wake_socket = std.os.socket(std.os.AF.INET, std.os.SOCK.STREAM, 0) catch 0;
    if (wake_socket != 0) {
        const address = std.net.Address.initIp4([_]u8{ 127, 0, 0, 1 }, 8080);
        _ = std.os.connect(wake_socket, &address.any, address.getOsSockLen()) catch {};
        std.os.closeSocket(wake_socket);
    }

    server_thread.join();

    const stats = server.getStats();
    const final_request_count = stats[0];
    const avg_response_time = stats[1];

    print("Zig HTTP Server Throughput Performance:\n");
    print("  Total requests handled: {}\n", .{final_request_count});
    print("  Average response time: {d:.2}μs\n", .{avg_response_time});
    print("  Requests per second: {d:.0}\n", .{requests_per_second});
    print("  Total elapsed time: {d:.2}s\n", .{elapsed});
}

fn benchmarkHttpServerLatency(allocator: Allocator) !void {
    print("Running Zig HTTP server latency benchmark...\n");

    // Test individual request latency
    var server = try HttpServer.init(allocator, 8081);
    defer server.deinit();

    const server_thread = try Thread.spawn(.{}, serverRunner, .{&server});

    std.time.sleep(200 * std.time.ns_per_ms);

    var latencies = ArrayList(f64).init(allocator);
    defer latencies.deinit();

    const iterations = 1000;
    var i: u32 = 0;
    while (i < iterations) : (i += 1) {
        const client_socket = std.os.socket(std.os.AF.INET, std.os.SOCK.STREAM, 0) catch continue;
        defer std.os.closeSocket(client_socket);

        const address = std.net.Address.initIp4([_]u8{ 127, 0, 0, 1 }, 8081);
        
        const start_time = std.time.nanoTimestamp();
        
        if (std.os.connect(client_socket, &address.any, address.getOsSockLen())) |_| {
            const request = "GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
            _ = std.os.send(client_socket, request, 0) catch continue;

            var buffer: [1024]u8 = undefined;
            if (std.os.recv(client_socket, &buffer, 0)) |_| {
                const end_time = std.time.nanoTimestamp();
                const latency_us = @intToFloat(f64, end_time - start_time) / 1000.0;
                try latencies.append(latency_us);
            } else |_| {}
        } else |_| {}
    }

    server.stop();

    // Connect once more to wake up the server
    const wake_socket = std.os.socket(std.os.AF.INET, std.os.SOCK.STREAM, 0) catch 0;
    if (wake_socket != 0) {
        const address = std.net.Address.initIp4([_]u8{ 127, 0, 0, 1 }, 8081);
        _ = std.os.connect(wake_socket, &address.any, address.getOsSockLen()) catch {};
        std.os.closeSocket(wake_socket);
    }

    server_thread.join();

    if (latencies.items.len > 0) {
        std.sort.sort(f64, latencies.items, {}, comptime std.sort.asc(f64));

        var sum: f64 = 0;
        for (latencies.items) |latency| {
            sum += latency;
        }
        const avg_latency = sum / @intToFloat(f64, latencies.items.len);
        
        const p95_index = @floatToInt(usize, @intToFloat(f64, latencies.items.len) * 0.95);
        const p99_index = @floatToInt(usize, @intToFloat(f64, latencies.items.len) * 0.99);
        
        const p95_latency = latencies.items[@min(p95_index, latencies.items.len - 1)];
        const p99_latency = latencies.items[@min(p99_index, latencies.items.len - 1)];

        print("Zig HTTP Server Latency Performance:\n");
        print("  Average latency: {d:.2}μs\n", .{avg_latency});
        print("  95th percentile: {d:.2}μs\n", .{p95_latency});
        print("  99th percentile: {d:.2}μs\n", .{p99_latency});
        print("  Total requests: {}\n", .{latencies.items.len});
    }
}

fn serverRunner(server: *HttpServer) void {
    server.start() catch |err| {
        print("Server error: {}\n", .{err});
    };
}

fn clientWorker(allocator: Allocator, client_id: usize, requests_per_client: u32) void {
    _ = allocator;
    
    var request_id: u32 = 0;
    while (request_id < requests_per_client) : (request_id += 1) {
        const client_socket = std.os.socket(std.os.AF.INET, std.os.SOCK.STREAM, 0) catch break;
        defer std.os.closeSocket(client_socket);

        const address = std.net.Address.initIp4([_]u8{ 127, 0, 0, 1 }, 8080);
        
        if (std.os.connect(client_socket, &address.any, address.getOsSockLen())) |_| {
            const request = std.fmt.allocPrint(allocator, 
                "GET /?client={}&request={} HTTP/1.1\r\n" ++
                "Host: localhost\r\n" ++
                "Connection: close\r\n" ++
                "\r\n", .{ client_id, request_id }) catch break;
            defer allocator.free(request);
            
            _ = std.os.send(client_socket, request, 0) catch break;

            var buffer: [1024]u8 = undefined;
            _ = std.os.recv(client_socket, &buffer, 0) catch {};
        } else |_| {
            break;
        }
    }
}

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    print("Running Zig HTTP Server Benchmarks...\n");

    benchmarkHttpServerThroughput(allocator) catch |err| {
        print("Error running throughput benchmark: {}\n", .{err});
        std.process.exit(1);
    };

    benchmarkHttpServerLatency(allocator) catch |err| {
        print("Error running latency benchmark: {}\n", .{err});
        std.process.exit(1);
    };

    print("Zig HTTP server benchmarks completed successfully!\n");
}