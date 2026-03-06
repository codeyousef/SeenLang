// Discrete Fourier Transform (Power Spectrum) Benchmark
// Faithful port of benchmarks/production/15_dft_spectrum.seen
// Same algorithm: naive O(N^2) DFT, N=8192, power spectrum in dB

use std::time::Instant;

fn run_dft(n: usize) -> f64 {
    let two_pi = 2.0 * std::f64::consts::PI;

    // Generate synthetic signal: sum of 3 sine waves
    let mut signal = vec![0.0f64; n];
    let mut i = 0;
    while i < n {
        let t = i as f64 / n as f64;
        signal[i] = 1.0 * (two_pi * 50.0 * t).sin()
            + 0.5 * (two_pi * 120.0 * t).sin()
            + 0.3 * (two_pi * 300.0 * t).sin();
        i += 1;
    }

    // Compute DFT
    let half_n = n / 2;
    let mut dft_re = vec![0.0f64; half_n];
    let mut dft_im = vec![0.0f64; half_n];

    let mut k = 0;
    while k < half_n {
        let mut re_sum = 0.0;
        let mut im_sum = 0.0;
        let mut j = 0;
        while j < n {
            let angle = two_pi * k as f64 * j as f64 / n as f64;
            re_sum += signal[j] * angle.cos();
            im_sum -= signal[j] * angle.sin();
            j += 1;
        }
        dft_re[k] = re_sum;
        dft_im[k] = im_sum;
        k += 1;
    }

    // Compute power spectrum in dB and phase
    let mut checksum = 0.0;
    let mut m = 0;
    while m < half_n {
        let power = dft_re[m] * dft_re[m] + dft_im[m] * dft_im[m];
        let mut power_db = 0.0;
        if power > 1e-9 {
            power_db = 10.0 * power.log10();
        }
        let phase = dft_im[m].atan2(dft_re[m]);
        checksum += power_db + phase;
        m += 1;
    }

    checksum
}

fn main() {
    let n = 8192;

    println!("DFT Power Spectrum Benchmark");
    println!("Signal length: {}", n);

    // Warmup
    let warmup_runs = 3;
    println!("Warming up ({} runs at n=512)...", warmup_runs);
    for _ in 0..warmup_runs {
        let _ = run_dft(512);
    }

    println!("Running measured iterations...");
    let iterations = 5;
    let mut min_time = f64::MAX;
    let mut result = 0.0;

    for _ in 0..iterations {
        let start = Instant::now();
        let checksum = run_dft(n);
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;

        if elapsed < min_time {
            min_time = elapsed;
            result = checksum;
        }
    }

    let trig_calls = n as f64 * n as f64;
    println!("Checksum: {:.9}", result);
    println!("Min time: {:.9} ms", min_time);
    println!(
        "Trig calls per second: {:.9} million",
        trig_calls / (min_time / 1000.0) / 1_000_000.0
    );
}
