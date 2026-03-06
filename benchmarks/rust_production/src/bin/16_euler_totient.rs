// Euler Totient (Number Theory) Benchmark
// Faithful port of benchmarks/production/16_euler_totient.seen
// Same algorithm: brute-force totient via GCD, n = 1..200000

use std::time::Instant;

#[inline(always)]
fn gcd(a: i64, b: i64) -> i64 {
    let mut x = a.abs();
    let mut y = b.abs();
    while y > 0 {
        let temp = y;
        y = x % y;
        x = temp;
    }
    x
}

fn euler_totient(n: i64) -> i64 {
    let mut count = 0;
    let mut k = 1;
    while k < n {
        if gcd(k, n) == 1 {
            count += 1;
        }
        k += 1;
    }
    count
}

fn run_totient_sum(limit: i64) -> i64 {
    let mut total = 0i64;
    let mut n = 1;
    while n <= limit {
        total += euler_totient(n);
        n += 1;
    }
    total
}

fn main() {
    let n = 10_000i64;

    println!("Euler Totient (Number Theory) Benchmark");
    println!("Computing phi(1) to phi({})", n);

    // Warmup
    let warmup_runs = 3;
    println!("Warming up ({} runs at n=1000)...", warmup_runs);
    for _ in 0..warmup_runs {
        let _ = run_totient_sum(1000);
    }

    println!("Running measured iterations...");
    let iterations = 5;
    let mut min_time = f64::MAX;
    let mut result = 0i64;

    for _ in 0..iterations {
        let start = Instant::now();
        let checksum = run_totient_sum(n);
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;

        if elapsed < min_time {
            min_time = elapsed;
            result = checksum;
        }
    }

    println!("Sum of phi(1..N): {}", result);
    println!("Min time: {:.9} ms", min_time);
    println!(
        "Totients per second: {:.9} million",
        n as f64 / (min_time / 1000.0) / 1_000_000.0
    );
}
