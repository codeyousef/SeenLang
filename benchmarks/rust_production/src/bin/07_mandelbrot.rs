// Mandelbrot Set Benchmark
// Faithful port of benchmarks/production/07_mandelbrot.seen
// Same algorithm: escape-time, 1000x1000, max_iter=100

use std::time::Instant;

fn mandelbrot_pixel(cx: f64, cy: f64, max_iter: i64) -> i64 {
    let mut zx = 0.0;
    let mut zy = 0.0;
    let mut iteration = 0i64;

    while iteration < max_iter {
        let zx2 = zx * zx;
        let zy2 = zy * zy;

        if zx2 + zy2 > 4.0 {
            return iteration;
        }

        let new_zy = 2.0 * zx * zy + cy;
        zx = zx2 - zy2 + cx;
        zy = new_zy;

        iteration += 1;
    }

    iteration
}

fn compute_mandelbrot(width: i64, height: i64, max_iter: i64) -> Vec<i64> {
    let mut pixels = Vec::with_capacity((width * height) as usize);

    let x_min: f64 = -2.5;
    let x_max: f64 = 1.0;
    let y_min: f64 = -1.0;
    let y_max: f64 = 1.0;

    let x_scale = (x_max - x_min) / width as f64;
    let y_scale = (y_max - y_min) / height as f64;

    let mut y = 0;
    while y < height {
        let mut x = 0;
        while x < width {
            let cx = x_min + x as f64 * x_scale;
            let cy = y_min + y as f64 * y_scale;

            let iter = mandelbrot_pixel(cx, cy, max_iter);
            pixels.push(iter);

            x += 1;
        }
        y += 1;
    }

    pixels
}

fn compute_checksum(pixels: &[i64]) -> i64 {
    pixels.iter().sum()
}

fn main() {
    let width = 1000i64;
    let height = 1000i64;
    let max_iter = 100i64;

    println!("Mandelbrot Set Benchmark");
    println!("Image size: {}x{}", width, height);
    println!("Max iterations: {}", max_iter);

    let warmup_runs = 3;
    println!("Warming up ({} runs)...", warmup_runs);
    for _ in 0..warmup_runs {
        let _ = compute_mandelbrot(width / 4, height / 4, max_iter);
    }

    println!("Running measured iterations...");
    let iterations = 5;
    let mut min_time = f64::MAX;
    let mut result_pixels = Vec::new();

    for _ in 0..iterations {
        let start = Instant::now();
        let pixels = compute_mandelbrot(width, height, max_iter);
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;

        if elapsed < min_time {
            min_time = elapsed;
            result_pixels = pixels;
        }
    }

    let checksum = compute_checksum(&result_pixels);
    let total_pixels = width * height;

    println!("Total pixels: {}", total_pixels);
    println!("Checksum: {}", checksum);
    println!("Min time: {:.9} ms", min_time);
    println!("Pixels per second: {:.9} million", total_pixels as f64 / (min_time / 1000.0) / 1_000_000.0);
}
