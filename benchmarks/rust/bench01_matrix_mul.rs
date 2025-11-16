use std::time::Instant;

const N: usize = 1920;
const TILE_SIZE: usize = 48;
const WARMUP_RUNS: usize = 5;
const MEASURED_RUNS: usize = 10;

fn matrix_multiply(a: &[f32], b: &[f32], c: &mut [f32]) {
    for i in 0..N {
        for k in 0..N {
            let a_ik = a[i * N + k];
            for j in 0..N {
                c[i * N + j] += a_ik * b[k * N + j];
            }
        }
    }
}

fn matrix_multiply_tiled(a: &[f32], b: &[f32], c: &mut [f32]) {
    for ii in (0..N).step_by(TILE_SIZE) {
        for kk in (0..N).step_by(TILE_SIZE) {
            for jj in (0..N).step_by(TILE_SIZE) {
                let i_max = (ii + TILE_SIZE).min(N);
                let k_max = (kk + TILE_SIZE).min(N);
                let j_max = (jj + TILE_SIZE).min(N);

                for i in ii..i_max {
                    for k in kk..k_max {
                        let a_ik = a[i * N + k];
                        for j in jj..j_max {
                            c[i * N + j] += a_ik * b[k * N + j];
                        }
                    }
                }
            }
        }
    }
}

fn main() {
    let mut rng_state: u64 = 42;
    let mut next_random = || -> f32 {
        rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
        ((rng_state / 65536) % 32768) as f32 / 32768.0
    };

    let mut a = vec![0.0f32; N * N];
    let mut b = vec![0.0f32; N * N];
    let mut c = vec![0.0f32; N * N];

    for i in 0..N * N {
        a[i] = next_random();
        b[i] = next_random();
    }

    for _ in 0..WARMUP_RUNS {
        c.fill(0.0);
        matrix_multiply_tiled(&a, &b, &mut c);
    }

    let mut times = Vec::with_capacity(MEASURED_RUNS);
    for _ in 0..MEASURED_RUNS {
        c.fill(0.0);
        let start = Instant::now();
        matrix_multiply_tiled(&a, &b, &mut c);
        let elapsed = start.elapsed();
        times.push(elapsed.as_secs_f64() * 1000.0);
    }

    let checksum: f64 = c.iter().map(|&x| x as f64).sum();
    let min_time = times.iter().cloned().fold(f64::INFINITY, f64::min);
    let gflops = (2.0 * (N as f64).powi(3)) / (min_time * 1e6);

    println!("Checksum: {:.6}", checksum);
    println!("Min time: {:.3} ms", min_time);
    println!("Performance: {:.2} GFLOPS", gflops);
}
