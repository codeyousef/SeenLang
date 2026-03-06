// Great-Circle Distance (Haversine) Benchmark
// Faithful port of benchmarks/production/13_great_circle.seen
// Same algorithm: golden ratio spiral coordinate generation, 2M pairs

use std::time::Instant;

fn haversine(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let earth_radius = 6371.0;
    let dlat = lat2 - lat1;
    let dlon = lon2 - lon1;
    let half_dlat = dlat / 2.0;
    let half_dlon = dlon / 2.0;
    let a = half_dlat.sin() * half_dlat.sin()
        + lat1.cos() * lat2.cos() * half_dlon.sin() * half_dlon.sin();
    let c = 2.0 * a.sqrt().asin();
    earth_radius * c
}

fn run_great_circle(n: usize) -> f64 {
    let pi = std::f64::consts::PI;
    let golden_ratio = 1.618033988749895_f64;
    let mut total_distance = 0.0;

    let mut i = 0;
    while i < n {
        // Point 1 from golden spiral
        let t1 = i as f64 / n as f64;
        let lat1 = (2.0 * t1 - 1.0).asin();
        let lon1 = 2.0 * pi * golden_ratio * i as f64;

        // Point 2 from offset golden spiral
        let j = i + 1;
        let t2 = j as f64 / n as f64;
        let lat2 = (2.0 * t2 - 1.0).asin();
        let lon2 = 2.0 * pi * golden_ratio * j as f64;

        let dist = haversine(lat1, lon1, lat2, lon2);
        total_distance += dist;

        i += 1;
    }

    total_distance
}

fn main() {
    let n = 2_000_000;

    println!("Great-Circle Distance (Haversine) Benchmark");
    println!("Pairs: {}", n);

    // Warmup
    let warmup_runs = 3;
    println!("Warming up ({} runs at n=40000)...", warmup_runs);
    for _ in 0..warmup_runs {
        let _ = run_great_circle(40000);
    }

    println!("Running measured iterations...");
    let iterations = 5;
    let mut min_time = f64::MAX;
    let mut result = 0.0;

    for _ in 0..iterations {
        let start = Instant::now();
        let checksum = run_great_circle(n);
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;

        if elapsed < min_time {
            min_time = elapsed;
            result = checksum;
        }
    }

    println!("Total distance: {:.9} km", result);
    println!("Min time: {:.9} ms", min_time);
    println!(
        "Pairs per second: {:.9} million",
        n as f64 / (min_time / 1000.0) / 1_000_000.0
    );
}
