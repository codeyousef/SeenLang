use std::time::Instant;
use rayon::prelude::*;

const SIZE: usize = 16000;
const MAX_ITER: usize = 10000;

fn mandelbrot(c_re: f64, c_im: f64) -> usize {
    let mut z_re = 0.0;
    let mut z_im = 0.0;
    let mut iter = 0;

    while iter < MAX_ITER && z_re * z_re + z_im * z_im <= 4.0 {
        let temp = z_re * z_re - z_im * z_im + c_re;
        z_im = 2.0 * z_re * z_im + c_im;
        z_re = temp;
        iter += 1;
    }

    iter
}

fn render() -> (Vec<u8>, u64) {
    let mut image = vec![0u8; SIZE * SIZE];

    let rows: Vec<Vec<u8>> = (0..SIZE)
        .into_par_iter()
        .map(|y| {
            let mut row = vec![0u8; SIZE];
            let im = -1.0 + 2.0 * (y as f64) / (SIZE as f64);

            for x in 0..SIZE {
                let re = -1.5 + 2.0 * (x as f64) / (SIZE as f64);
                row[x] = (mandelbrot(re, im) % 256) as u8;
            }

            row
        })
        .collect();

    for (y, row) in rows.iter().enumerate() {
        image[y * SIZE..(y + 1) * SIZE].copy_from_slice(row);
    }

    let checksum: u64 = image.iter().map(|&x| x as u64).sum();

    (image, checksum)
}

fn main() {
    let mut times = Vec::new();
    let mut final_checksum = 0;

    for _ in 0..5 {
        let start = Instant::now();
        let (_image, checksum) = render();
        let elapsed = start.elapsed();

        times.push(elapsed.as_secs_f64());
        final_checksum = checksum;
    }

    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = times[times.len() / 2];

    println!("Mandelbrot Set ({}x{}, max_iter={})", SIZE, SIZE, MAX_ITER);
    println!("Checksum: {}", final_checksum);
    println!("Median time: {:.3} s", median);
}
