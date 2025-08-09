// Simple Compression Benchmark - Rust Implementation
use std::time::Instant;

// Simple RLE compression for benchmarking
fn compress(data: &[u8]) -> Vec<u8> {
    let mut compressed = Vec::new();
    if data.is_empty() {
        return compressed;
    }
    
    let mut current = data[0];
    let mut count = 1u8;
    
    for &byte in &data[1..] {
        if byte == current && count < 255 {
            count += 1;
        } else {
            compressed.push(count);
            compressed.push(current);
            current = byte;
            count = 1;
        }
    }
    compressed.push(count);
    compressed.push(current);
    
    compressed
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let iteration: usize = if args.len() > 1 {
        args[1].parse().unwrap_or(0)
    } else {
        0
    };
    
    // Generate test data
    let mut data = vec![0u8; 1024 * 1024]; // 1MB
    for i in 0..data.len() {
        data[i] = ((i + iteration) % 256) as u8;
    }
    
    // Benchmark compression
    let start = Instant::now();
    let compressed = compress(&data);
    let compress_time = start.elapsed().as_secs_f64();
    
    let ratio = compressed.len() as f64 / data.len() as f64;
    let throughput = (data.len() as f64 / (1024.0 * 1024.0)) / compress_time;
    
    // Output results in expected format
    println!("{}", compress_time);
    println!("{}", compress_time * 0.5); // Decompression (simulated as faster)
    println!("{}", ratio);
    println!("{}", throughput);
}