use std::time::Instant;

const N: usize = 1920;
const TILE_SIZE: usize = 48;

fn random_matrix(seed: u64) -> Vec<f32> {
    let mut rng = seed;
    let mut matrix = vec![0.0f32; N * N];
    for i in 0..N * N {
        rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
        matrix[i] = ((rng / 65536) % 32768) as f32 / 32768.0;
    }
    matrix
}

fn matrix_multiply(a: &[f32], b: &[f32]) -> Vec<f32> {
    let mut c = vec![0.0f32; N * N];

    for ii in (0..N).step_by(TILE_SIZE) {
        for kk in (0..N).step_by(TILE_SIZE) {
            for jj in (0..N).step_by(TILE_SIZE) {
                let i_end = (ii + TILE_SIZE).min(N);
                let k_end = (kk + TILE_SIZE).min(N);
                let j_end = (jj + TILE_SIZE).min(N);

                for i in ii..i_end {
                    for k in kk..k_end {
                        let a_ik = a[i * N + k];
                        for j in jj..j_end {
                            c[i * N + j] += a_ik * b[k * N + j];
                        }
                    }
                }
            }
        }
    }

    c
}

fn main() {
    let a = random_matrix(42);
    let b = random_matrix(84);

    for _ in 0..5 {
        let _ = matrix_multiply(&a, &b);
    }

    let mut times = Vec::new();
    for _ in 0..10 {
        let start = Instant::now();
        let c = matrix_multiply(&a, &b);
        let elapsed = start.elapsed();
        times.push(elapsed.as_secs_f64());

        let checksum: f64 = c.iter().map(|&x| x as f64).sum();
        if times.len() == 10 {
            println!("Checksum: {}", checksum);
        }
    }

    let min_time = times.iter().copied().fold(f64::INFINITY, f64::min);
    let gflops = (2.0 * (N as f64).powi(3)) / (min_time * 1e9);

    println!("Matrix Multiplication ({}x{})", N, N);
    println!("Min time: {:.3} ms", min_time * 1000.0);
    println!("GFLOPS: {:.2}", gflops);
}
