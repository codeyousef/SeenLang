// Benchmark 1: Simple Loop and Sum
// Measures: Basic integer arithmetic and loop performance

use std::time::Instant;

fn sum_loop(n: i64) -> i64 {
    let mut sum = 0i64;
    for i in 0..n {
        sum += i;
    }
    sum
}

fn main() {
    const N: i64 = 100_000_000;
    const ITERATIONS: usize = 10;

    println!("Benchmark 1: Loop and Sum");
    println!("Summing 0..{}", N);

    // Warmup
    for _ in 0..3 {
        let _ = sum_loop(N);
    }

    // Measured runs
    let mut times = Vec::new();
    for _ in 0..ITERATIONS {
        let start = Instant::now();
        let result = sum_loop(N);
        let elapsed = start.elapsed();
        times.push(elapsed.as_secs_f64() * 1000.0);

        // Verify result
        let expected = (N * (N - 1)) / 2;
        if result != expected {
            eprintln!("ERROR: Expected {}, got {}", expected, result);
        }
    }

    let avg = times.iter().sum::<f64>() / times.len() as f64;
    let min = times.iter().fold(f64::INFINITY, |a, &b| a.min(b));

    println!("Result: {} (verified)", (N * (N - 1)) / 2);
    println!("Average time: {:.2} ms", avg);
    println!("Min time: {:.2} ms", min);
}
