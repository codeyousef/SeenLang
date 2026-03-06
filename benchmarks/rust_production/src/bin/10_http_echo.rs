// HTTP Echo Server Benchmark
// Simulated HTTP request/response processing
// Same algorithm as Seen: build HTTP response string for each request, sum lengths

use std::hint::black_box;
use std::time::Instant;

fn process_request(body: &str) -> String {
    let mut response = String::with_capacity(200);
    response.push_str("HTTP/1.1 200 OK\r\n");
    response.push_str("Content-Type: application/json\r\n");
    response.push_str("Server: Seen/1.0\r\n");
    response.push_str("\r\n");
    response.push_str("{\"echo\":\"");
    response.push_str(body);
    response.push_str("\"}");
    response
}

fn benchmark_http(n: usize) -> i64 {
    let test_data = "{\"user\":\"test\",\"action\":\"ping\",\"timestamp\":1234567890}";
    let mut total_length: i64 = 0;
    for _ in 0..n {
        let response = process_request(test_data);
        total_length += black_box(response).len() as i64;
    }
    total_length
}

fn main() {
    let n: usize = 5_000_000;

    println!("HTTP Echo Server Benchmark");
    println!("Requests to process: {}", n);

    let warmup_runs = 3;
    println!("Warming up ({} runs)...", warmup_runs);
    for _ in 0..warmup_runs {
        let _ = benchmark_http(n / 10);
    }

    println!("Running measured iterations...");
    let iterations = 5;
    let mut min_time = f64::MAX;
    let mut result_length: i64 = 0;

    for _ in 0..iterations {
        let start = Instant::now();
        let total_length = benchmark_http(n);
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;

        if elapsed < min_time {
            min_time = elapsed;
            result_length = total_length;
        }
    }

    println!("Checksum: {}", result_length);
    println!("Min time: {:.9} ms", min_time);
    println!(
        "Requests per second: {:.9} thousand",
        n as f64 / (min_time / 1000.0) / 1000.0
    );
    println!(
        "Throughput: {:.9} MB/s",
        result_length as f64 / (min_time / 1000.0) / 1_000_000.0
    );
}
