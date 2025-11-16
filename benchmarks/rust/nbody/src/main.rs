use std::time::Instant;

const SOLAR_MASS: f64 = 4.0 * std::f64::consts::PI * std::f64::consts::PI;
const DAYS_PER_YEAR: f64 = 365.24;
const N_BODIES: usize = 5;
const N_STEPS: usize = 50_000_000;
const DT: f64 = 0.01;

#[derive(Clone, Copy)]
struct Body {
    x: f64,
    y: f64,
    z: f64,
    vx: f64,
    vy: f64,
    vz: f64,
    mass: f64,
}

fn init_bodies() -> [Body; N_BODIES] {
    [
        Body {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            vx: 0.0,
            vy: 0.0,
            vz: 0.0,
            mass: SOLAR_MASS,
        },
        Body {
            x: 4.84143144246472090e+00,
            y: -1.16032004402742839e+00,
            z: -1.03622044471123109e-01,
            vx: 1.66007664274403694e-03 * DAYS_PER_YEAR,
            vy: 7.69901118419740425e-03 * DAYS_PER_YEAR,
            vz: -6.90460016972063023e-05 * DAYS_PER_YEAR,
            mass: 9.54791938424326609e-04 * SOLAR_MASS,
        },
        Body {
            x: 8.34336671824457987e+00,
            y: 4.12479856412430479e+00,
            z: -4.03523417114321381e-01,
            vx: -2.76742510726862411e-03 * DAYS_PER_YEAR,
            vy: 4.99852801234917238e-03 * DAYS_PER_YEAR,
            vz: 2.30417297573763929e-05 * DAYS_PER_YEAR,
            mass: 2.85885980666130812e-04 * SOLAR_MASS,
        },
        Body {
            x: 1.28943695621391310e+01,
            y: -1.51111514016986312e+01,
            z: -2.23307578892655734e-01,
            vx: 2.96460137564761618e-03 * DAYS_PER_YEAR,
            vy: 2.37847173959480950e-03 * DAYS_PER_YEAR,
            vz: -2.96589568540237556e-05 * DAYS_PER_YEAR,
            mass: 4.36624404335156298e-05 * SOLAR_MASS,
        },
        Body {
            x: 1.53796971148509165e+01,
            y: -2.59193146099879641e+01,
            z: 1.79258772950371181e-01,
            vx: 2.68067772490389322e-03 * DAYS_PER_YEAR,
            vy: 1.62824170038242295e-03 * DAYS_PER_YEAR,
            vz: -9.51592254519715870e-05 * DAYS_PER_YEAR,
            mass: 5.15138902046611451e-05 * SOLAR_MASS,
        },
    ]
}

fn energy(bodies: &[Body; N_BODIES]) -> f64 {
    let mut e = 0.0;

    for i in 0..N_BODIES {
        e += 0.5 * bodies[i].mass * (
            bodies[i].vx * bodies[i].vx +
                bodies[i].vy * bodies[i].vy +
                bodies[i].vz * bodies[i].vz
        );

        for j in (i + 1)..N_BODIES {
            let dx = bodies[i].x - bodies[j].x;
            let dy = bodies[i].y - bodies[j].y;
            let dz = bodies[i].z - bodies[j].z;
            let distance = (dx * dx + dy * dy + dz * dz).sqrt();
            e -= (bodies[i].mass * bodies[j].mass) / distance;
        }
    }

    e
}

fn advance(bodies: &mut [Body; N_BODIES], dt: f64) {
    for i in 0..N_BODIES {
        for j in (i + 1)..N_BODIES {
            let dx = bodies[i].x - bodies[j].x;
            let dy = bodies[i].y - bodies[j].y;
            let dz = bodies[i].z - bodies[j].z;

            let dist_sq = dx * dx + dy * dy + dz * dz;
            let distance = dist_sq.sqrt();
            let mag = dt / (dist_sq * distance);

            bodies[i].vx -= dx * bodies[j].mass * mag;
            bodies[i].vy -= dy * bodies[j].mass * mag;
            bodies[i].vz -= dz * bodies[j].mass * mag;

            bodies[j].vx += dx * bodies[i].mass * mag;
            bodies[j].vy += dy * bodies[i].mass * mag;
            bodies[j].vz += dz * bodies[i].mass * mag;
        }
    }

    for body in bodies.iter_mut() {
        body.x += dt * body.vx;
        body.y += dt * body.vy;
        body.z += dt * body.vz;
    }
}

fn main() {
    let mut bodies = init_bodies();

    let mut px = 0.0;
    let mut py = 0.0;
    let mut pz = 0.0;
    for body in &bodies[1..] {
        px += body.vx * body.mass;
        py += body.vy * body.mass;
        pz += body.vz * body.mass;
    }
    bodies[0].vx = -px / SOLAR_MASS;
    bodies[0].vy = -py / SOLAR_MASS;
    bodies[0].vz = -pz / SOLAR_MASS;

    let initial_energy = energy(&bodies);

    let start = Instant::now();
    for _ in 0..N_STEPS {
        advance(&mut bodies, DT);
    }
    let elapsed = start.elapsed();

    let final_energy = energy(&bodies);

    println!("N-Body Simulation (n={}, steps={})", N_BODIES, N_STEPS);
    println!("Initial energy: {:.9}", initial_energy);
    println!("Final energy: {:.9}", final_energy);
    println!("Energy conservation error: {:.2e}", (final_energy - initial_energy).abs());
    println!("Time: {:.3} s", elapsed.as_secs_f64());
}
