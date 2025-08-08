// Rust HTTP server benchmark implementation for fair comparison with Seen
// High-performance HTTP server using similar architecture to Seen implementation

use std::collections::HashMap;
use std::io::{Read, Write, BufReader, BufRead};
use std::net::{TcpListener, TcpStream, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};
use std::fs;
use std::str;

// HTTP Request representation
#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

impl HttpRequest {
    pub fn new() -> Self {
        HttpRequest {
            method: String::new(),
            path: String::new(),
            version: String::new(),
            headers: HashMap::new(),
            body: String::new(),
        }
    }

    pub fn parse_from_buffer(buffer: &str) -> Result<HttpRequest, String> {
        let lines: Vec<&str> = buffer.split('\n').collect();
        if lines.is_empty() {
            return Err("Empty request".to_string());
        }

        let mut request = HttpRequest::new();

        // Parse request line (GET /path HTTP/1.1)
        let request_line_parts: Vec<&str> = lines[0].split(' ').collect();
        if request_line_parts.len() != 3 {
            return Err("Invalid request line".to_string());
        }

        request.method = request_line_parts[0].to_string();
        request.path = request_line_parts[1].to_string();
        request.version = request_line_parts[2].trim().to_string();

        // Parse headers
        let mut i = 1;
        while i < lines.len() && !lines[i].trim().is_empty() {
            if let Some(colon_pos) = lines[i].find(':') {
                let key = lines[i][0..colon_pos].trim().to_lowercase();
                let value = lines[i][colon_pos + 1..].trim().to_string();
                request.headers.insert(key, value);
            }
            i += 1;
        }

        // Parse body (if present)
        i += 1; // Skip empty line
        if i < lines.len() {
            let body_lines = &lines[i..];
            request.body = body_lines.join("\n");
        }

        Ok(request)
    }

    pub fn get_header(&self, name: &str) -> Option<&String> {
        self.headers.get(&name.to_lowercase())
    }
}

// HTTP Response representation
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub version: String,
    pub status_code: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

impl HttpResponse {
    pub fn new(status_code: u16, status_text: String) -> Self {
        HttpResponse {
            version: "HTTP/1.1".to_string(),
            status_code,
            status_text,
            headers: HashMap::new(),
            body: String::new(),
        }
    }

    pub fn ok(body: String) -> Self {
        let mut response = HttpResponse::new(200, "OK".to_string());
        response.set_body(body);
        response
    }

    pub fn not_found() -> Self {
        let mut response = HttpResponse::new(404, "Not Found".to_string());
        response.set_body("<html><body><h1>404 Not Found</h1></body></html>".to_string());
        response.set_header("Content-Type".to_string(), "text/html".to_string());
        response
    }

    pub fn internal_server_error() -> Self {
        let mut response = HttpResponse::new(500, "Internal Server Error".to_string());
        response.set_body("<html><body><h1>500 Internal Server Error</h1></body></html>".to_string());
        response.set_header("Content-Type".to_string(), "text/html".to_string());
        response
    }

    pub fn bad_request() -> Self {
        let mut response = HttpResponse::new(400, "Bad Request".to_string());
        response.set_body("<html><body><h1>400 Bad Request</h1></body></html>".to_string());
        response.set_header("Content-Type".to_string(), "text/html".to_string());
        response
    }

    pub fn set_header(&mut self, key: String, value: String) {
        self.headers.insert(key, value);
    }

    pub fn set_body(&mut self, body: String) {
        self.body = body;
        self.set_header("Content-Length".to_string(), body.len().to_string());
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut response = format!("{} {} {}\r\n", self.version, self.status_code, self.status_text);

        for (key, value) in &self.headers {
            response.push_str(&format!("{}: {}\r\n", key, value));
        }

        response.push_str("\r\n");
        response.push_str(&self.body);

        response.into_bytes()
    }
}

// Route handler type
pub type RouteHandler = fn(&HttpRequest) -> HttpResponse;

// Simple HTTP Server implementation
pub struct HttpServer {
    listener: TcpListener,
    running: Arc<AtomicBool>,
    request_count: Arc<AtomicU64>,
    response_time_sum: Arc<AtomicU64>, // in microseconds
    routes: HashMap<String, RouteHandler>,
}

impl HttpServer {
    pub fn new(port: u16) -> Result<Self, String> {
        let address = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(&address)
            .map_err(|e| format!("Failed to bind to {}: {}", address, e))?;

        listener.set_nonblocking(false)
            .map_err(|e| format!("Failed to set blocking mode: {}", e))?;

        Ok(HttpServer {
            listener,
            running: Arc::new(AtomicBool::new(false)),
            request_count: Arc::new(AtomicU64::new(0)),
            response_time_sum: Arc::new(AtomicU64::new(0)),
            routes: HashMap::new(),
        })
    }

    pub fn add_route(&mut self, path: String, handler: RouteHandler) {
        self.routes.insert(path, handler);
    }

    fn handle_request(&self, request: &HttpRequest) -> HttpResponse {
        let start_time = Instant::now();

        let response = if let Some(handler) = self.routes.get(&request.path) {
            handler(request)
        } else {
            match request.path.as_str() {
                "/" => HttpResponse::ok("Hello, World!".to_string()),
                "/health" => {
                    let mut response = HttpResponse::ok("OK".to_string());
                    response.set_header("Content-Type".to_string(), "text/plain".to_string());
                    response
                }
                "/stats" => {
                    let request_count = self.request_count.load(Ordering::Relaxed);
                    let avg_response_time = if request_count > 0 {
                        self.response_time_sum.load(Ordering::Relaxed) / request_count
                    } else {
                        0
                    };

                    let stats = format!(
                        r#"{{"requests": {}, "avgResponseTimeUs": {}}}"#,
                        request_count, avg_response_time
                    );

                    let mut response = HttpResponse::ok(stats);
                    response.set_header("Content-Type".to_string(), "application/json".to_string());
                    response
                }
                "/echo" => {
                    let mut response = HttpResponse::ok(request.body.clone());
                    response.set_header("Content-Type".to_string(), "text/plain".to_string());
                    response
                }
                path if path.starts_with("/static/") => {
                    // Simple static file serving
                    let file_name = &path[8..]; // Remove "/static/" prefix
                    self.serve_static_file(file_name)
                }
                _ => HttpResponse::not_found(),
            }
        };

        let elapsed = start_time.elapsed();
        let response_time = elapsed.as_micros() as u64;

        self.request_count.fetch_add(1, Ordering::Relaxed);
        self.response_time_sum.fetch_add(response_time, Ordering::Relaxed);

        response
    }

    fn serve_static_file(&self, file_name: &str) -> HttpResponse {
        // Security: basic path traversal protection
        if file_name.contains("..") || file_name.contains("/") {
            return HttpResponse::not_found();
        }

        let file_path = format!("static/{}", file_name);

        match fs::read_to_string(&file_path) {
            Ok(content) => {
                let mut response = HttpResponse::ok(content);

                // Set content type based on file extension
                let content_type = match file_name.split('.').last() {
                    Some("html") => "text/html",
                    Some("css") => "text/css",
                    Some("js") => "application/javascript",
                    Some("json") => "application/json",
                    Some("txt") => "text/plain",
                    _ => "application/octet-stream",
                };

                response.set_header("Content-Type".to_string(), content_type.to_string());
                response
            }
            Err(_) => HttpResponse::not_found(),
        }
    }

    fn handle_connection(&self, mut stream: TcpStream) -> Result<(), String> {
        let mut buffer = [0u8; 4096];

        match stream.read(&mut buffer) {
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    return Ok(()); // Connection closed by client
                }

                let request_data = String::from_utf8_lossy(&buffer[..bytes_read]);

                match HttpRequest::parse_from_buffer(&request_data) {
                    Ok(request) => {
                        let response = self.handle_request(&request);
                        let response_bytes = response.to_bytes();

                        if let Err(e) = stream.write_all(&response_bytes) {
                            return Err(format!("Failed to write response: {}", e));
                        }

                        if let Err(e) = stream.flush() {
                            return Err(format!("Failed to flush stream: {}", e));
                        }
                    }
                    Err(_) => {
                        // Send 400 Bad Request for malformed requests
                        let response = HttpResponse::bad_request();
                        let response_bytes = response.to_bytes();
                        let _ = stream.write_all(&response_bytes);
                    }
                }
            }
            Err(e) => {
                return Err(format!("Failed to read from stream: {}", e));
            }
        }

        Ok(())
    }

    pub fn start(&self) -> Result<(), String> {
        self.running.store(true, Ordering::Relaxed);

        println!("HTTP Server started on {}", self.listener.local_addr().unwrap());

        let thread_pool = ThreadPool::new(4); // 4 worker threads

        for stream in self.listener.incoming() {
            if !self.running.load(Ordering::Relaxed) {
                break;
            }

            match stream {
                Ok(stream) => {
                    let server_data = ServerData {
                        routes: self.routes.clone(),
                        request_count: self.request_count.clone(),
                        response_time_sum: self.response_time_sum.clone(),
                    };

                    thread_pool.execute(move || {
                        let temp_server = TempServer(server_data);
                        if let Err(e) = temp_server.0.handle_connection(stream) {
                            eprintln!("Error handling connection: {}", e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Error accepting connection: {}", e);
                    continue;
                }
            }
        }

        Ok(())
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }

    pub fn get_stats(&self) -> (u64, f64) {
        let request_count = self.request_count.load(Ordering::Relaxed);
        let avg_response_time = if request_count > 0 {
            self.response_time_sum.load(Ordering::Relaxed) as f64 / request_count as f64
        } else {
            0.0
        };

        (request_count, avg_response_time)
    }

    pub fn get_running_flag(&self) -> Arc<AtomicBool> {
        self.running.clone()
    }

    pub fn get_request_count(&self) -> Arc<AtomicU64> {
        self.request_count.clone()
    }

    pub fn get_response_time_sum(&self) -> Arc<AtomicU64> {
        self.response_time_sum.clone()
    }
}

// Helper structures for thread pool
#[derive(Clone)]
struct ServerData {
    routes: HashMap<String, RouteHandler>,
    request_count: Arc<AtomicU64>,
    response_time_sum: Arc<AtomicU64>,
}

struct TempServer(ServerData);

impl TempServer {
    fn handle_connection(&self, stream: TcpStream) -> Result<(), String> {
        let server = HttpServer {
            listener: TcpListener::bind("127.0.0.1:0").unwrap(), // Dummy listener
            running: Arc::new(AtomicBool::new(true)),
            request_count: self.0.request_count.clone(),
            response_time_sum: self.0.response_time_sum.clone(),
            routes: self.0.routes.clone(),
        };
        server.handle_connection(stream)
    }
}

// Thread pool for handling concurrent connections
pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: mpsc::Sender<Job>,
}

type Job = Box<dyn FnOnce() + Send + 'static>;

impl ThreadPool {
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        ThreadPool { workers, sender }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.sender.send(job).unwrap();
    }
}

struct Worker {
    _id: usize,
    _thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            let job = receiver.lock().unwrap().recv().unwrap();
            job();
        });

        Worker {
            _id: id,
            _thread: Some(thread),
        }
    }
}

// Benchmark functions
pub fn benchmark_http_server_throughput() -> Result<(), Box<dyn std::error::Error>> {
    println!("Running Rust HTTP server throughput benchmark...");

    // Start server in background thread
    let server = HttpServer::new(8080)?;
    let server_running = server.get_running_flag();
    let server_request_count = server.get_request_count();
    let server_response_time_sum = server.get_response_time_sum();

    let server_thread = thread::spawn(move || {
        server.start().expect("Failed to start server");
    });

    // Wait for server to start
    thread::sleep(Duration::from_millis(100));

    // Run concurrent load test
    let start_time = Instant::now();
    let number_of_clients = 50;
    let requests_per_client = 100;
    let mut handles = Vec::new();

    for client_id in 0..number_of_clients {
        let handle = thread::spawn(move || {
            for request_id in 0..requests_per_client {
                match TcpStream::connect("127.0.0.1:8080") {
                    Ok(mut stream) => {
                        let request = format!(
                            "GET /?client={}&request={} HTTP/1.1\r\n\
                             Host: localhost\r\n\
                             Connection: close\r\n\
                             \r\n",
                            client_id, request_id
                        );

                        if stream.write_all(request.as_bytes()).is_err() {
                            break;
                        }

                        let mut buffer = [0u8; 1024];
                        let _ = stream.read(&mut buffer);
                    }
                    Err(_) => break,
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all clients to complete
    for handle in handles {
        let _ = handle.join();
    }

    let end_time = Instant::now();
    let elapsed = end_time.duration_since(start_time);
    let total_requests = number_of_clients * requests_per_client;
    let requests_per_second = total_requests as f64 / elapsed.as_secs_f64();

    // Stop server
    server_running.store(false, Ordering::Relaxed);
    let _ = TcpStream::connect("127.0.0.1:8080"); // Wake up server to check running flag
    let _ = server_thread.join();

    let final_request_count = server_request_count.load(Ordering::Relaxed);
    let avg_response_time = if final_request_count > 0 {
        server_response_time_sum.load(Ordering::Relaxed) as f64 / final_request_count as f64
    } else {
        0.0
    };

    println!("Rust HTTP Server Throughput Performance:");
    println!("  Total requests handled: {}", final_request_count);
    println!("  Average response time: {:.2}μs", avg_response_time);
    println!("  Requests per second: {:.0}", requests_per_second);
    println!("  Total elapsed time: {:.2}s", elapsed.as_secs_f64());

    Ok(())
}

pub fn benchmark_http_server_latency() -> Result<(), Box<dyn std::error::Error>> {
    println!("Running Rust HTTP server latency benchmark...");

    // Test individual request latency
    let server = HttpServer::new(8081)?;
    let server_running = server.get_running_flag();

    let server_thread = thread::spawn(move || {
        server.start().expect("Failed to start server");
    });

    thread::sleep(Duration::from_millis(100));

    let mut latencies = Vec::new();
    let iterations = 1000;

    for _ in 0..iterations {
        match TcpStream::connect("127.0.0.1:8081") {
            Ok(mut stream) => {
                let start_time = Instant::now();

                let request = "GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";

                if stream.write_all(request.as_bytes()).is_ok() {
                    let mut buffer = [0u8; 1024];
                    if stream.read(&mut buffer).is_ok() {
                        let end_time = Instant::now();
                        let latency_us = end_time.duration_since(start_time).as_micros() as f64;
                        latencies.push(latency_us);
                    }
                }
            }
            Err(e) => {
                println!("Failed to connect: {}", e);
            }
        }
    }

    server_running.store(false, Ordering::Relaxed);
    let _ = TcpStream::connect("127.0.0.1:8081");
    let _ = server_thread.join();

    if !latencies.is_empty() {
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let avg_latency = latencies.iter().sum::<f64>() / latencies.len() as f64;
        let p95_index = ((latencies.len() as f64 * 0.95) as usize).min(latencies.len() - 1);
        let p99_index = ((latencies.len() as f64 * 0.99) as usize).min(latencies.len() - 1);
        let p95_latency = latencies[p95_index];
        let p99_latency = latencies[p99_index];

        println!("Rust HTTP Server Latency Performance:");
        println!("  Average latency: {:.2}μs", avg_latency);
        println!("  95th percentile: {:.2}μs", p95_latency);
        println!("  99th percentile: {:.2}μs", p99_latency);
        println!("  Total requests: {}", latencies.len());
    }

    Ok(())
}

fn main() {
    println!("Running Rust HTTP Server Benchmarks...");

    if let Err(e) = benchmark_http_server_throughput() {
        eprintln!("Error running throughput benchmark: {}", e);
        std::process::exit(1);
    }

    if let Err(e) = benchmark_http_server_latency() {
        eprintln!("Error running latency benchmark: {}", e);
        std::process::exit(1);
    }

    println!("Rust HTTP server benchmarks completed successfully!");
}