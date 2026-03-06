// Fannkuch-Redux Benchmark
// Classic CLBG benchmark: generates all permutations and counts pancake flips
// Tests integer arrays, permutations, branching

use std::time::Instant;

fn run_fannkuch(n: usize) -> (i64, i64) {
    let mut perm = vec![0i64; n];
    let mut perm1 = vec![0i64; n];
    let mut count = vec![0i64; n];

    for i in 0..n {
        perm1[i] = i as i64;
    }

    let mut max_flips = 0i64;
    let mut checksum = 0i64;
    let mut perm_count = 0i64;
    let mut r = n;

    loop {
        while r != 1 {
            count[r - 1] = r as i64;
            r -= 1;
        }

        for i in 0..n {
            perm[i] = perm1[i];
        }

        // Count flips
        let mut flips = 0i64;
        let mut k = perm[0] as usize;
        while k != 0 {
            let mut i = 0;
            let mut j = k;
            while i < j {
                perm.swap(i, j);
                i += 1;
                j -= 1;
            }
            flips += 1;
            k = perm[0] as usize;
        }

        if flips > max_flips {
            max_flips = flips;
        }

        if perm_count % 2 == 0 {
            checksum += flips;
        } else {
            checksum -= flips;
        }

        perm_count += 1;

        // Generate next permutation (Heap's algorithm variant)
        r = 1;
        loop {
            if r >= n {
                return (checksum, max_flips);
            }

            let perm0 = perm1[0];
            let mut i = 0;
            while i < r {
                perm1[i] = perm1[i + 1];
                i += 1;
            }
            perm1[r] = perm0;

            count[r] -= 1;
            if count[r] > 0 {
                break;
            }
            r += 1;
        }
    }
}

fn main() {
    let n = 12;

    println!("Fannkuch-Redux Benchmark");
    println!("N: {}", n);

    // Warmup
    println!("Warming up (1 run at n=10)...");
    let _ = run_fannkuch(10);

    println!("Running measured iterations...");
    let iterations = 3;
    let mut min_time = f64::MAX;
    let mut result_checksum = 0i64;
    let mut result_max_flips = 0i64;

    for _ in 0..iterations {
        let start = Instant::now();
        let (checksum, max_flips) = run_fannkuch(n);
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;

        if elapsed < min_time {
            min_time = elapsed;
            result_checksum = checksum;
            result_max_flips = max_flips;
        }
    }

    println!("Checksum: {}", result_checksum);
    println!("Max flips: {}", result_max_flips);
    println!("Min time: {:.9} ms", min_time);
}
