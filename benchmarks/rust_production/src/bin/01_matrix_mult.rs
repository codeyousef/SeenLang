// Matrix Multiplication Benchmark (SGEMM)
// Faithful port of benchmarks/production/01_matrix_mult.seen
// Same algorithm: cache-blocked matrix multiply with block_size=64, size=512

use std::time::Instant;

fn matrix_new(size: usize) -> Vec<f64> {
    vec![0.0; size * size]
}

fn matrix_fill_random(data: &mut [f64], seed: i64) {
    let mut current_seed = seed;
    for val in data.iter_mut() {
        current_seed = (current_seed.wrapping_mul(1103515245) + 12345) % 2147483647;
        if current_seed < 0 {
            current_seed = -current_seed;
        }
        let value_int = current_seed % 10000;
        *val = value_int as f64 / 10000.0;
    }
}

fn matrix_multiply(a: &[f64], b: &[f64], c: &mut [f64], size: usize) {
    let block_size = 64;

    let mut ii = 0;
    while ii < size {
        let mut jj = 0;
        while jj < size {
            let mut kk = 0;
            while kk < size {
                let i_end = (ii + block_size).min(size);
                let j_end = (jj + block_size).min(size);
                let k_end = (kk + block_size).min(size);

                let mut i = ii;
                while i < i_end {
                    let mut j = jj;
                    while j < j_end {
                        let mut sum = c[i * size + j];
                        let mut k = kk;
                        while k < k_end {
                            sum += a[i * size + k] * b[k * size + j];
                            k += 1;
                        }
                        c[i * size + j] = sum;
                        j += 1;
                    }
                    i += 1;
                }

                kk += block_size;
            }
            jj += block_size;
        }
        ii += block_size;
    }
}

fn compute_checksum(data: &[f64]) -> f64 {
    data.iter().sum()
}

fn main() {
    let size = 1024;

    println!("Matrix Multiplication Benchmark (SGEMM)");
    println!("Matrix size: {}x{}", size, size);

    let mut a = matrix_new(size);
    let mut b = matrix_new(size);
    let mut c = matrix_new(size);

    matrix_fill_random(&mut a, 12345);
    matrix_fill_random(&mut b, 67890);

    let warmup_runs = 3;
    println!("Warming up ({} runs)...", warmup_runs);
    for _ in 0..warmup_runs {
        matrix_multiply(&a, &b, &mut c, size);
    }

    println!("Running measured iterations...");
    let iterations = 5;
    let mut min_time = f64::MAX;

    for _ in 0..iterations {
        let mut c_fresh = matrix_new(size);

        let start = Instant::now();
        matrix_multiply(&a, &b, &mut c_fresh, size);
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;

        if elapsed < min_time {
            min_time = elapsed;
        }
    }

    let checksum = compute_checksum(&c);
    let size_f = size as f64;
    let gflops = (2.0 * size_f * size_f * size_f) / (min_time / 1000.0) / 1_000_000_000.0;

    println!("Checksum: {}", checksum);
    println!("Min time: {:.9} ms", min_time);
    println!("Performance: {:.9} GFLOPS", gflops);
}
