use std::time::Instant;

fn a(i: usize, j: usize) -> f64 {
    1.0 / ((i + j) * (i + j + 1) / 2 + i + 1) as f64
}

fn multiply_at_av(v: &[f64], at_av: &mut [f64], n: usize) {
    let mut u = vec![0.0; n];
    
    // Multiply by A
    for i in 0..n {
        u[i] = 0.0;
        for j in 0..n {
            u[i] += a(i, j) * v[j];
        }
    }
    
    // Multiply by A transpose
    for i in 0..n {
        at_av[i] = 0.0;
        for j in 0..n {
            at_av[i] += a(j, i) * u[j];
        }
    }
}

fn main() {
    let start = Instant::now();
    
    let n = 100;
    let mut u = vec![1.0; n];
    let mut v = vec![0.0; n];
    
    for _ in 0..10 {
        multiply_at_av(&u, &mut v, n);
        multiply_at_av(&v, &mut u, n);
    }
    
    let mut v_bv = 0.0;
    let mut vv = 0.0;
    for i in 0..n {
        v_bv += u[i] * v[i];
        vv += v[i] * v[i];
    }
    
    let norm = (v_bv / vv).sqrt();
    
    let duration = start.elapsed();
    let duration_ms = duration.as_secs_f64() * 1000.0;
    
    let flops_per_sec = (n * n * 20) as f64 / duration.as_secs_f64();
    let memory_mb = (n * 2 * std::mem::size_of::<f64>()) as f64 / (1024.0 * 1024.0);
    
    // Output: computation_time_ms flops_per_sec memory_mb spectral_norm
    println!("{} {} {} {}", duration_ms, flops_per_sec, memory_mb, norm);
}