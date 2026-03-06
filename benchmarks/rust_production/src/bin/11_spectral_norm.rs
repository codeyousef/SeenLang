// Spectral Norm Benchmark
// Classic CLBG benchmark: eigenvalue approximation of infinite matrix
// Tests dense float math, O(n^2) nested loops, function inlining

use std::time::Instant;

#[inline(always)]
fn eval_a(i: usize, j: usize) -> f64 {
    1.0 / ((i + j) * (i + j + 1) / 2 + i + 1) as f64
}

fn multiply_av(v: &[f64], out: &mut [f64], n: usize) {
    let mut i = 0;
    while i < n {
        let mut sum = 0.0;
        let mut j = 0;
        while j < n {
            sum += eval_a(i, j) * v[j];
            j += 1;
        }
        out[i] = sum;
        i += 1;
    }
}

fn multiply_atv(v: &[f64], out: &mut [f64], n: usize) {
    let mut i = 0;
    while i < n {
        let mut sum = 0.0;
        let mut j = 0;
        while j < n {
            sum += eval_a(j, i) * v[j];
            j += 1;
        }
        out[i] = sum;
        i += 1;
    }
}

fn multiply_atav(v: &[f64], out: &mut [f64], tmp: &mut [f64], n: usize) {
    multiply_av(v, tmp, n);
    multiply_atv(tmp, out, n);
}

fn run_spectral_norm(n: usize) -> f64 {
    let mut u = vec![1.0f64; n];
    let mut v = vec![0.0f64; n];
    let mut tmp = vec![0.0f64; n];

    for _ in 0..10 {
        multiply_atav(&u, &mut v, &mut tmp, n);
        multiply_atav(&v, &mut u, &mut tmp, n);
    }

    let mut vbv = 0.0;
    let mut vv = 0.0;
    let mut i = 0;
    while i < n {
        vbv += u[i] * v[i];
        vv += v[i] * v[i];
        i += 1;
    }

    (vbv / vv).sqrt()
}

fn main() {
    let n = 5500;

    println!("Spectral Norm Benchmark");
    println!("N: {}", n);

    // Warmup
    let warmup_runs = 3;
    println!("Warming up ({} runs at n=500)...", warmup_runs);
    for _ in 0..warmup_runs {
        let _ = run_spectral_norm(500);
    }

    println!("Running measured iterations...");
    let iterations = 5;
    let mut min_time = f64::MAX;
    let mut result = 0.0f64;

    for _ in 0..iterations {
        let start = Instant::now();
        let norm = run_spectral_norm(n);
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;

        if elapsed < min_time {
            min_time = elapsed;
            result = norm;
        }
    }

    // 10 iterations * 2 AtAv per iteration * 2 mat-vec per AtAv = 40 mat-vec multiplies
    // Each mat-vec is n*n multiply-adds = 2*n*n FLOPs
    // Total: 40 * 2 * n * n FLOPs, plus dot products (negligible)
    let flops = 40.0 * 2.0 * (n as f64) * (n as f64);
    let mflops = flops / (min_time / 1000.0) / 1_000_000.0;

    println!("Spectral norm: {:.9}", result);
    println!("Min time: {:.9} ms", min_time);
    println!("Throughput: {:.9} MFLOPS", mflops);
}
