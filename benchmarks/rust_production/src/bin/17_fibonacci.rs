// Fibonacci Benchmark
// Compute sum of first N fibonacci numbers using iterative method
// Tests loop performance and integer arithmetic
// N = 80000000, expected checksum depends on mod arithmetic (i64 overflow wraps)
use std::time::Instant;

fn benchmark_fib(n: i64) -> i64 {
    let mut a: i64 = 0;
    let mut b: i64 = 1;
    let mut sum: i64 = 0;
    for _ in 0..n {
        sum = sum.wrapping_add(a);
        let c = a.wrapping_add(b);
        a = b;
        b = c;
    }
    sum
}

fn main() {
    let n: i64 = 1000000000;

    println!("Fibonacci Benchmark");
    println!("Computing sum of first {} fibonacci numbers", n);

    let warmup_runs = 3;
    println!("Warming up ({} runs)...", warmup_runs);
    for _ in 0..warmup_runs {
        std::hint::black_box(benchmark_fib(n / 10));
    }

    println!("Running measured iterations...");
    let iterations = 5;
    let mut min_time = f64::MAX;
    let mut result: i64 = 0;

    for _ in 0..iterations {
        let start = Instant::now();
        let answer = std::hint::black_box(benchmark_fib(n));
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        if elapsed < min_time {
            min_time = elapsed;
            result = answer;
        }
    }

    println!("Checksum: {}", result);
    println!("Min time: {:.6} ms", min_time);
}
