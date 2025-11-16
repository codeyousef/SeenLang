// Simple Fibonacci Benchmark
// Baseline performance comparison

use std::time::Instant;

fn fibonacci(n: u64) -> u64 {
    if n <= 1 {
        return n;
    }
    fibonacci(n - 1) + fibonacci(n - 2)
}

fn fibonacci_iterative(n: u64) -> u64 {
    if n <= 1 {
        return n;
    }

    let mut a = 0;
    let mut b = 1;

    for _ in 2..=n {
        let temp = a + b;
        a = b;
        b = temp;
    }

    b
}

fn main() {
    const N: u64 = 35;
    const ITERATIONS: usize = 5;

    println!("Simple Fibonacci Benchmark");
    println!("Computing fib({})", N);

    // Warmup
    for _ in 0..3 {
        let _ = fibonacci_iterative(N);
    }

    // Measured runs - iterative
    let mut times_iter = Vec::new();
    for _ in 0..ITERATIONS {
        let start = Instant::now();
        let result = fibonacci_iterative(N);
        let elapsed = start.elapsed();
        times_iter.push(elapsed.as_secs_f64() * 1_000_000.0); // microseconds

        if result != 9227465 {
            eprintln!("ERROR: Wrong result {}", result);
        }
    }

    let avg_iter = times_iter.iter().sum::<f64>() / times_iter.len() as f64;

    println!("Iterative version:");
    println!("  Result: 9227465");
    println!("  Average time: {:.2} μs", avg_iter);
    println!("  Min time: {:.2} μs", times_iter.iter().fold(f64::INFINITY, |a, &b| a.min(b)));

    // Warmup recursive
    for _ in 0..2 {
        let _ = fibonacci(25);
    }

    // Measured runs - recursive (smaller N)
    let n_rec = 30;
    let mut times_rec = Vec::new();
    for _ in 0..ITERATIONS {
        let start = Instant::now();
        let result = fibonacci(n_rec);
        let elapsed = start.elapsed();
        times_rec.push(elapsed.as_secs_f64() * 1000.0); // milliseconds

        if result != 832040 {
            eprintln!("ERROR: Wrong result {}", result);
        }
    }

    let avg_rec = times_rec.iter().sum::<f64>() / times_rec.len() as f64;

    println!("\nRecursive version (fib({})):", n_rec);
    println!("  Result: 832040");
    println!("  Average time: {:.2} ms", avg_rec);
    println!("  Min time: {:.2} ms", times_rec.iter().fold(f64::INFINITY, |a, &b| a.min(b)));
}
