// Benchmark 4: Array Operations
// Measures: Array allocation, iteration, and memory access

use std::time::Instant;

fn array_sum(size: usize) -> (i64, f64) {
    // Allocate array
    let mut arr = vec![0i64; size];

    // Fill array
    for i in 0..size {
        arr[i] = i as i64;
    }

    // Sum array
    let mut sum = 0i64;
    for &val in &arr {
        sum += val;
    }

    let memory = (size * std::mem::size_of::<i64>()) as f64 / (1024.0 * 1024.0);
    (sum, memory)
}

fn main() {
    const SIZE: usize = 10_000_000;
    const ITERATIONS: usize = 10;

    println!("Benchmark 4: Array Operations");
    println!("Array size: {} elements", SIZE);

    // Warmup
    for _ in 0..3 {
        let _ = array_sum(SIZE);
    }

    // Measured runs
    let mut times = Vec::new();
    let mut memory_mb = 0.0;

    for _ in 0..ITERATIONS {
        let start = Instant::now();
        let (sum, mem) = array_sum(SIZE);
        let elapsed = start.elapsed();
        times.push(elapsed.as_secs_f64() * 1000.0);
        memory_mb = mem;

        // Verify
        let expected = ((SIZE - 1) as i64 * SIZE as i64) / 2;
        if sum != expected {
            eprintln!("ERROR: Expected {}, got {}", expected, sum);
        }
    }

    let avg = times.iter().sum::<f64>() / times.len() as f64;
    let min = times.iter().fold(f64::INFINITY, |a, &b| a.min(b));

    println!("Sum: {} (verified)", ((SIZE - 1) as i64 * SIZE as i64) / 2);
    println!("Memory: {:.2} MB", memory_mb);
    println!("Average time: {:.2} ms", avg);
    println!("Min time: {:.2} ms", min);
}
