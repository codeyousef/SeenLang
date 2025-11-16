use std::time::Instant;

const N: usize = 10_000_000;
const SEGMENT_SIZE: usize = 262_144;

fn sieve_of_eratosthenes() -> (usize, Vec<usize>) {
    let limit = ((N as f64).sqrt() as usize) + 1;
    let mut is_prime = vec![true; limit + 1];
    is_prime[0] = false;
    is_prime[1] = false;

    for i in 2..=((limit as f64).sqrt() as usize) {
        if is_prime[i] {
            for j in (i * i..=limit).step_by(i) {
                is_prime[j] = false;
            }
        }
    }

    let base_primes: Vec<usize> = (2..=limit).filter(|&p| is_prime[p]).collect();

    let mut all_primes = base_primes.clone();
    let mut low = limit + 1;

    while low <= N {
        let high = (low + SEGMENT_SIZE - 1).min(N);
        let mut segment = vec![true; high - low + 1];

        for &p in &base_primes {
            let mut start = ((low + p - 1) / p) * p;
            if start < p * p {
                start = p * p;
            }

            for j in (start..=high).step_by(p) {
                segment[j - low] = false;
            }
        }

        for i in 0..segment.len() {
            if segment[i] {
                all_primes.push(low + i);
            }
        }

        low = high + 1;
    }

    (all_primes.len(), all_primes)
}

fn main() {
    for _ in 0..3 {
        let _ = sieve_of_eratosthenes();
    }

    let start = Instant::now();
    let (count, primes) = sieve_of_eratosthenes();
    let elapsed = start.elapsed();

    let prime_str = primes.iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let checksum = format!("{:x}", md5::compute(prime_str.as_bytes()));

    println!("Sieve of Eratosthenes (n={})", N);
    println!("Prime count: {} (expected: 664579)", count);
    println!("MD5 checksum: {}", checksum);
    println!("Time: {:.3} ms", elapsed.as_secs_f64() * 1000.0);
}
