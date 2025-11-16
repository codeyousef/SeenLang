use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

const WIDTH: usize = 4000;
const HEIGHT: usize = 4000;
const MAX_ITER: u32 = 1000;
const THREADS: usize = 8;

fn mandelbrot(cx: f64, cy: f64, max_iter: u32) -> u32 {
    let mut x = 0.0;
    let mut y = 0.0;
    let mut iter = 0;

    while x * x + y * y <= 4.0 && iter < max_iter {
        let xtemp = x * x - y * y + cx;
        y = 2.0 * x * y + cy;
        x = xtemp;
        iter += 1;
    }

    iter
}

fn render_chunk(start_row: usize, end_row: usize, pixels: Arc<Mutex<Vec<u32>>>) -> u64 {
    let mut local_sum = 0u64;
    let mut local_pixels = Vec::new();

    for py in start_row..end_row {
        for px in 0..WIDTH {
            let x0 = -1.5 + (px as f64 / WIDTH as f64) * 2.0;
            let y0 = -1.0 + (py as f64 / HEIGHT as f64) * 2.0;
            let iter = mandelbrot(x0, y0, MAX_ITER);
            local_pixels.push(iter);
            local_sum += iter as u64;
        }
    }

    let mut pixels = pixels.lock().unwrap();
    pixels.extend(local_pixels);
    local_sum
}

fn main() {
    let warmup_runs = 2;
    for _ in 0..warmup_runs {
        let pixels = Arc::new(Mutex::new(Vec::new()));
        let rows_per_thread = HEIGHT / THREADS;
        let mut handles = vec![];

        for t in 0..THREADS {
            let pixels = pixels.clone();
            let start_row = t * rows_per_thread;
            let end_row = if t == THREADS - 1 { HEIGHT } else { (t + 1) * rows_per_thread };

            handles.push(thread::spawn(move || {
                render_chunk(start_row, end_row, pixels)
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    let measured_runs = 5;
    let mut times = Vec::with_capacity(measured_runs);
    let mut final_checksum = 0u64;

    for run in 0..measured_runs {
        let pixels = Arc::new(Mutex::new(Vec::new()));
        let rows_per_thread = HEIGHT / THREADS;
        let mut handles = vec![];

        let start = Instant::now();

        for t in 0..THREADS {
            let pixels = pixels.clone();
            let start_row = t * rows_per_thread;
            let end_row = if t == THREADS - 1 { HEIGHT } else { (t + 1) * rows_per_thread };

            handles.push(thread::spawn(move || {
                render_chunk(start_row, end_row, pixels)
            }));
        }

        let mut checksum = 0u64;
        for handle in handles {
            checksum += handle.join().unwrap();
        }

        let elapsed = start.elapsed();
        times.push(elapsed.as_secs_f64() * 1000.0);

        if run == 0 {
            final_checksum = checksum;
        }
    }

    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_time = times[measured_runs / 2];

    println!("Checksum: {}", final_checksum);
    println!("Median time: {:.3} ms", median_time);
    println!("Resolution: {}x{}", WIDTH, HEIGHT);
    println!("Threads: {}", THREADS);
}
