// N-Body Simulation Benchmark
// Faithful port of benchmarks/production/05_nbody.seen
// Same algorithm: parallel arrays for body properties, 50M steps

use std::time::Instant;

fn advance(
    bx: &mut [f64], by: &mut [f64], bz: &mut [f64],
    bvx: &mut [f64], bvy: &mut [f64], bvz: &mut [f64],
    bmass: &[f64], dt: f64,
) {
    let n = bx.len();

    let mut i = 0;
    while i < n {
        let mut j = i + 1;
        while j < n {
            let dx = bx[i] - bx[j];
            let dy = by[i] - by[j];
            let dz = bz[i] - bz[j];

            let dist_sq = dx * dx + dy * dy + dz * dz;
            let dist = dist_sq.sqrt();
            let mag = dt / (dist_sq * dist);

            bvx[i] -= dx * bmass[j] * mag;
            bvy[i] -= dy * bmass[j] * mag;
            bvz[i] -= dz * bmass[j] * mag;

            bvx[j] += dx * bmass[i] * mag;
            bvy[j] += dy * bmass[i] * mag;
            bvz[j] += dz * bmass[i] * mag;

            j += 1;
        }
        i += 1;
    }

    for k in 0..n {
        bx[k] += dt * bvx[k];
        by[k] += dt * bvy[k];
        bz[k] += dt * bvz[k];
    }
}

fn energy(
    bx: &[f64], by: &[f64], bz: &[f64],
    bvx: &[f64], bvy: &[f64], bvz: &[f64],
    bmass: &[f64],
) -> f64 {
    let mut e = 0.0;
    let n = bx.len();

    let mut i = 0;
    while i < n {
        e += 0.5 * bmass[i] * (bvx[i] * bvx[i] + bvy[i] * bvy[i] + bvz[i] * bvz[i]);

        let mut j = i + 1;
        while j < n {
            let dx = bx[i] - bx[j];
            let dy = by[i] - by[j];
            let dz = bz[i] - bz[j];

            let distance = (dx * dx + dy * dy + dz * dz).sqrt();
            e -= (bmass[i] * bmass[j]) / distance;

            j += 1;
        }
        i += 1;
    }

    e
}

fn offset_momentum(
    bvx: &mut [f64], bvy: &mut [f64], bvz: &mut [f64],
    bmass: &[f64],
) {
    let mut px = 0.0;
    let mut py = 0.0;
    let mut pz = 0.0;

    for i in 0..bmass.len() {
        px += bvx[i] * bmass[i];
        py += bvy[i] * bmass[i];
        pz += bvz[i] * bmass[i];
    }

    let solar_mass = 4.0 * std::f64::consts::PI * std::f64::consts::PI;
    bvx[0] = -px / solar_mass;
    bvy[0] = -py / solar_mass;
    bvz[0] = -pz / solar_mass;
}

fn init_bodies(
    bx: &mut [f64], by: &mut [f64], bz: &mut [f64],
    bvx: &mut [f64], bvy: &mut [f64], bvz: &mut [f64],
    bmass: &mut [f64], solar_mass: f64, days_per_year: f64,
) {
    // Sun
    bx[0] = 0.0; by[0] = 0.0; bz[0] = 0.0;
    bvx[0] = 0.0; bvy[0] = 0.0; bvz[0] = 0.0;
    bmass[0] = solar_mass;
    // Jupiter
    bx[1] = 4.84143144246472090e+00;
    by[1] = -1.16032004402742839e+00;
    bz[1] = -1.03622044471123109e-01;
    bvx[1] = 1.66007664274403694e-03 * days_per_year;
    bvy[1] = 7.69901118419740425e-03 * days_per_year;
    bvz[1] = -6.90460016972063023e-05 * days_per_year;
    bmass[1] = 9.54791938424326609e-04 * solar_mass;
    // Saturn
    bx[2] = 8.34336671824457987e+00;
    by[2] = 4.12479856412430479e+00;
    bz[2] = -4.03523417114321381e-01;
    bvx[2] = -2.76742510726862411e-03 * days_per_year;
    bvy[2] = 4.99852801234917238e-03 * days_per_year;
    bvz[2] = 2.30417297573763929e-05 * days_per_year;
    bmass[2] = 2.85885980666130812e-04 * solar_mass;
    // Uranus
    bx[3] = 1.28943695621391310e+01;
    by[3] = -1.51111514016986312e+01;
    bz[3] = -2.23307578892655734e-01;
    bvx[3] = 2.96460137564761618e-03 * days_per_year;
    bvy[3] = 2.37847173959480950e-03 * days_per_year;
    bvz[3] = -2.96589568540237556e-05 * days_per_year;
    bmass[3] = 4.36624404335156298e-05 * solar_mass;
    // Neptune
    bx[4] = 1.53796971148509165e+01;
    by[4] = -2.59193146099879641e+01;
    bz[4] = 1.79258772950371181e-01;
    bvx[4] = 2.68067772490389322e-03 * days_per_year;
    bvy[4] = 1.62824170038242295e-03 * days_per_year;
    bvz[4] = -9.51592254519715870e-05 * days_per_year;
    bmass[4] = 5.15138902046611451e-05 * solar_mass;
}

fn main() {
    let solar_mass = 4.0 * std::f64::consts::PI * std::f64::consts::PI;
    let days_per_year = 365.24;
    let num_steps = 50_000_000;

    println!("N-Body Simulation Benchmark");
    println!("Simulating {} steps", num_steps);

    let mut bx = vec![0.0; 5];
    let mut by = vec![0.0; 5];
    let mut bz = vec![0.0; 5];
    let mut bvx = vec![0.0; 5];
    let mut bvy = vec![0.0; 5];
    let mut bvz = vec![0.0; 5];
    let mut bmass = vec![0.0; 5];

    let warmup_runs = 3;
    println!("Warming up ({} runs at n/100)...", warmup_runs);
    for _ in 0..warmup_runs {
        init_bodies(&mut bx, &mut by, &mut bz, &mut bvx, &mut bvy, &mut bvz, &mut bmass, solar_mass, days_per_year);
        offset_momentum(&mut bvx, &mut bvy, &mut bvz, &bmass);
        for _ in 0..(num_steps / 100) {
            advance(&mut bx, &mut by, &mut bz, &mut bvx, &mut bvy, &mut bvz, &bmass, 0.01);
        }
    }

    println!("Running measured iterations...");
    let iterations = 3;
    let mut min_time = f64::MAX;
    let mut checksum = 0.0;

    for _ in 0..iterations {
        init_bodies(&mut bx, &mut by, &mut bz, &mut bvx, &mut bvy, &mut bvz, &mut bmass, solar_mass, days_per_year);
        offset_momentum(&mut bvx, &mut bvy, &mut bvz, &bmass);

        let start = Instant::now();
        for _ in 0..num_steps {
            advance(&mut bx, &mut by, &mut bz, &mut bvx, &mut bvy, &mut bvz, &bmass, 0.01);
        }
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;

        if elapsed < min_time {
            min_time = elapsed;
        }
        checksum = energy(&bx, &by, &bz, &bvx, &bvy, &bvz, &bmass);
    }

    println!("Checksum: {:.9}", checksum);
    println!("Min time: {:.9} ms", min_time);
    println!("Steps per second: {:.9} million", num_steps as f64 / (min_time / 1000.0) / 1_000_000.0);
}
