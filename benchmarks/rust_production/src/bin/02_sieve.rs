// Sieve of Eratosthenes Benchmark
// Faithful port of benchmarks/production/02_sieve.seen
// Same algorithm: basic sieve with Vec<i64> flags, limit=100000

use std::time::Instant;

fn sieve_of_eratosthenes(limit: usize) -> Vec<i64> {
    let mut is_prime = vec![1i64; limit + 1];

    is_prime[0] = 0;
    is_prime[1] = 0;

    let mut p = 2;
    while p * p <= limit {
        if is_prime[p] != 0 {
            let mut j = p * p;
            while j <= limit {
                is_prime[j] = 0;
                j += p;
            }
        }
        p += 1;
    }

    let mut primes = Vec::new();
    for idx in 2..=limit {
        if is_prime[idx] != 0 {
            primes.push(idx as i64);
        }
    }

    primes
}

fn compute_checksum(primes: &[i64]) -> i64 {
    primes.iter().sum()
}

fn main() {
    let limit = 10000000;

    println!("Sieve of Eratosthenes Benchmark");
    println!("Finding primes up to: {}", limit);

    let warmup_runs = 3;
    println!("Warming up ({} runs)...", warmup_runs);
    for _ in 0..warmup_runs {
        let _ = sieve_of_eratosthenes(limit);
    }

    println!("Running measured iterations...");
    let iterations = 5;
    let mut min_time = f64::MAX;
    let mut result_primes = Vec::new();

    for _ in 0..iterations {
        let start = Instant::now();
        let primes = sieve_of_eratosthenes(limit);
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;

        if elapsed < min_time {
            min_time = elapsed;
            result_primes = primes;
        }
    }

    let prime_count = result_primes.len();
    let checksum = compute_checksum(&result_primes);

    println!("Prime count: {}", prime_count);
    println!("Checksum: {}", checksum);
    println!("Min time: {:.9} ms", min_time);
}
