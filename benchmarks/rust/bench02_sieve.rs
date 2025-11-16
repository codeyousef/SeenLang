// Benchmark 2: Sieve of Eratosthenes
// Measures: Memory access patterns, branch prediction, integer arithmetic

use std::time::Instant;

fn sieve_of_eratosthenes(n: usize) -> (usize, Vec<u32>) {
    // Bit-packed boolean array (1 bit per number)
    let size = (n + 63) / 64;
    let mut sieve = vec![!0u64; size];

    // 0 and 1 are not prime
    sieve[0] &= !0b11;

    let limit = (n as f64).sqrt() as usize + 1;

    // Wheel factorization: skip multiples of 2, 3, 5
    let wheel = [1, 7, 11, 13, 17, 19, 23, 29];

    for i in (3..=limit).step_by(2) {
        let word = i / 64;
        let bit = i % 64;

        if (sieve[word] >> bit) & 1 == 1 {
            // Mark multiples starting at i*i
            let mut j = i * i;
            while j <= n {
                let word = j / 64;
                let bit = j % 64;
                sieve[word] &= !(1u64 << bit);
                j += i * 2; // Skip even multiples
            }
        }
    }

    // Count primes and collect them
    let mut primes = Vec::with_capacity(700000);
    primes.push(2);

    for i in (3..=n).step_by(2) {
        let word = i / 64;
        let bit = i % 64;
        if (sieve[word] >> bit) & 1 == 1 {
            primes.push(i as u32);
        }
    }

    (primes.len(), primes)
}

fn md5_checksum(primes: &[u32]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    for &p in primes {
        p.hash(&mut hasher);
    }
    format!("{:x}", hasher.finish())
}

fn main() {
    const N: usize = 10_000_000;

    // Warmup
    for _ in 0..3 {
        let _ = sieve_of_eratosthenes(N);
    }

    // Measured run
    let start = Instant::now();
    let (count, primes) = sieve_of_eratosthenes(N);
    let elapsed = start.elapsed();

    let checksum = md5_checksum(&primes);

    println!("Benchmark 2: Sieve of Eratosthenes");
    println!("n = {}", N);
    println!("Prime count: {} (expected 664579)", count);
    println!("Checksum: {}", checksum);
    println!("Time: {:.2} ms", elapsed.as_secs_f64() * 1000.0);
}
