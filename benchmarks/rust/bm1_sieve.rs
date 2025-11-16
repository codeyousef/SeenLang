// BM1: Sieve of Eratosthenes - Prime Generation
// Specification: Generate all primes up to 10,000,000

use std::time::Instant;

fn main() {
    let n = 10_000_000;
    let warmup = 3;

    // Warmup runs
    for _ in 0..warmup {
        sieve_of_eratosthenes(n);
    }

    // Measured run
    let start = Instant::now();
    let prime_count = sieve_of_eratosthenes(n);
    let elapsed = start.elapsed();

    println!("Sieve of Eratosthenes Benchmark");
    println!("================================");
    println!("Range: 1 to {}", n);
    println!("Prime count: {}", prime_count);
    println!("Expected: 664579");
    println!("Time: {} ms", elapsed.as_millis());
    print!("Verification: ");
    if prime_count == 664579 {
        println!("PASS");
    } else {
        println!("FAIL");
    }
}

fn sieve_of_eratosthenes(n: usize) -> usize {
    if n < 2 {
        return 0;
    }

    let limit = int_sqrt(n) + 1;
    let mut is_prime = vec![true; limit];
    is_prime[0] = false;
    is_prime[1] = false;

    // Mark composites in base sieve
    for i in 2..limit {
        if is_prime[i] {
            let mut j = i * i;
            while j < limit {
                is_prime[j] = false;
                j += i;
            }
        }
    }

    // Collect base primes
    let base_primes: Vec<usize> = is_prime
        .iter()
        .enumerate()
        .filter_map(|(i, &is_p)| if is_p { Some(i) } else { None })
        .collect();

    // Count primes in base
    let mut prime_count = base_primes.len();

    // Segmented sieve
    let segment_size = 32768;
    let mut low = limit;

    while low < n {
        let high = (low + segment_size).min(n);
        let mut segment = vec![true; segment_size];

        for &prime in &base_primes {
            let mut start = ((low + prime - 1) / prime) * prime;
            if start < prime * prime {
                start = prime * prime;
            }

            let mut j = start;
            while j < high {
                segment[j - low] = false;
                j += prime;
            }
        }

        // Count primes in segment
        for i in low..high {
            if segment[i - low] {
                prime_count += 1;
            }
        }

        low = high;
    }

    prime_count
}

fn int_sqrt(n: usize) -> usize {
    if n < 2 {
        return n;
    }

    let mut x = n;
    let mut y = (x + 1) / 2;

    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }

    x
}
