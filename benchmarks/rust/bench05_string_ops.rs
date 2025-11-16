// Benchmark 5: String Operations
// Measures: String concatenation and allocation

use std::time::Instant;

fn string_concat(count: usize) -> (String, usize) {
    let mut result = String::new();

    for i in 0..count {
        result.push_str(&format!("Item_{} ", i));
    }

    let len = result.len();
    (result, len)
}

fn main() {
    const COUNT: usize = 10_000;
    const ITERATIONS: usize = 10;

    println!("Benchmark 5: String Operations");
    println!("Concatenating {} strings", COUNT);

    // Warmup
    for _ in 0..3 {
        let _ = string_concat(COUNT);
    }

    // Measured runs
    let mut times = Vec::new();
    let mut final_len = 0;

    for _ in 0..ITERATIONS {
        let start = Instant::now();
        let (result, len) = string_concat(COUNT);
        let elapsed = start.elapsed();
        times.push(elapsed.as_secs_f64() * 1000.0);
        final_len = len;

        // Prevent dead code elimination
        if result.is_empty() {
            eprintln!("ERROR: Empty result");
        }
    }

    let avg = times.iter().sum::<f64>() / times.len() as f64;
    let min = times.iter().fold(f64::INFINITY, |a, &b| a.min(b));

    println!("Final length: {} bytes", final_len);
    println!("Average time: {:.2} ms", avg);
    println!("Min time: {:.2} ms", min);
}
