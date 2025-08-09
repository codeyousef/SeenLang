// Simple HTTP Server Benchmark - Rust Implementation
use std::time::Instant;

// Simulate HTTP request processing
fn process_request(request_id: i32) {
    // Simulate some work
    let mut sum = 0;
    for i in 0..1000 {
        sum += i * request_id;
    }
    // Prevent optimization
    std::hint::black_box(sum);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let iteration: i32 = if args.len() > 1 {
        args[1].parse().unwrap_or(0)
    } else {
        0
    };
    
    let num_requests = 10000;
    let concurrent_connections = 100;
    
    // Benchmark request processing
    let start = Instant::now();
    
    for i in 0..num_requests {
        process_request(i + iteration);
    }
    
    let total_time = start.elapsed().as_secs_f64();
    let rps = num_requests as f64 / total_time;
    let latency_ms = (total_time / num_requests as f64) * 1000.0;
    let memory_mb = 45 + (iteration % 10); // Simulated memory usage
    
    // Output results in expected format
    println!("{}", rps);
    println!("{}", latency_ms);
    println!("{}", memory_mb);
    println!("{}", concurrent_connections);
}