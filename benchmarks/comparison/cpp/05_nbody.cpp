// N-Body Simulation Benchmark
// Same algorithm as Seen: parallel arrays for body properties, 50M steps
#include <cstdio>
#include <cstdint>
#include <cmath>
#include <chrono>

constexpr int N_BODIES = 5;
constexpr double PI = 3.141592653589793;

static void advance(double* bx, double* by, double* bz,
                    double* bvx, double* bvy, double* bvz,
                    const double* bmass, double dt, int n) {
    for (int i = 0; i < n; i++) {
        for (int j = i + 1; j < n; j++) {
            double dx = bx[i] - bx[j];
            double dy = by[i] - by[j];
            double dz = bz[i] - bz[j];
            double dist_sq = dx*dx + dy*dy + dz*dz;
            double dist = std::sqrt(dist_sq);
            double mag = dt / (dist_sq * dist);
            double mj_mag = bmass[j] * mag;
            double mi_mag = bmass[i] * mag;
            bvx[i] -= dx * mj_mag; bvy[i] -= dy * mj_mag; bvz[i] -= dz * mj_mag;
            bvx[j] += dx * mi_mag; bvy[j] += dy * mi_mag; bvz[j] += dz * mi_mag;
        }
    }
    for (int k = 0; k < n; k++) {
        bx[k] += dt * bvx[k];
        by[k] += dt * bvy[k];
        bz[k] += dt * bvz[k];
    }
}

static double energy(const double* bx, const double* by, const double* bz,
                     const double* bvx, const double* bvy, const double* bvz,
                     const double* bmass, int n) {
    double e = 0.0;
    for (int i = 0; i < n; i++) {
        e += 0.5 * bmass[i] * (bvx[i]*bvx[i] + bvy[i]*bvy[i] + bvz[i]*bvz[i]);
        for (int j = i + 1; j < n; j++) {
            double dx = bx[i] - bx[j];
            double dy = by[i] - by[j];
            double dz = bz[i] - bz[j];
            e -= (bmass[i] * bmass[j]) / std::sqrt(dx*dx + dy*dy + dz*dz);
        }
    }
    return e;
}

static void offset_momentum(double* bvx, double* bvy, double* bvz,
                             const double* bmass, int n, double solar_mass) {
    double px = 0, py = 0, pz = 0;
    for (int i = 0; i < n; i++) {
        px += bvx[i] * bmass[i];
        py += bvy[i] * bmass[i];
        pz += bvz[i] * bmass[i];
    }
    bvx[0] = -px / solar_mass;
    bvy[0] = -py / solar_mass;
    bvz[0] = -pz / solar_mass;
}

int main() {
    double solar_mass = 4.0 * PI * PI;
    double days_per_year = 365.24;
    double dt = 0.01;
    int n = N_BODIES;

    printf("N-Body Simulation Benchmark\n");

    double bx[N_BODIES]  = {0.0, 4.84143144246472090e+00, 8.34336671824457987e+00, 1.28943695621391310e+01, 1.53796971148509165e+01};
    double by[N_BODIES]  = {0.0, -1.16032004402742839e+00, 4.12479856412430479e+00, -1.51111514016986312e+01, -2.59193146099879641e+01};
    double bz[N_BODIES]  = {0.0, -1.03622044471123109e-01, -4.03523417114321381e-01, -2.23307578892655734e-01, 1.79258772950371181e-01};
    double bvx[N_BODIES] = {0.0, 1.66007664274403694e-03*days_per_year, -2.76742510726862411e-03*days_per_year, 2.96460137564761618e-03*days_per_year, 2.68067772490389322e-03*days_per_year};
    double bvy[N_BODIES] = {0.0, 7.69901118419740425e-03*days_per_year, 4.99852801234917238e-03*days_per_year, 2.37847173959480950e-03*days_per_year, 1.62824170038242295e-03*days_per_year};
    double bvz[N_BODIES] = {0.0, -6.90460016972063023e-05*days_per_year, 2.30417297573763929e-05*days_per_year, -2.96589568540237556e-05*days_per_year, -9.51592254519715870e-05*days_per_year};
    double bmass[N_BODIES] = {solar_mass, 9.54791938424326609e-04*solar_mass, 2.85885980666130812e-04*solar_mass, 4.36624404335156298e-05*solar_mass, 5.15138902046611451e-05*solar_mass};

    offset_momentum(bvx, bvy, bvz, bmass, n, solar_mass);

    int64_t num_steps = 50000000;
    printf("Simulating %ld steps\n", (long)num_steps);

    int warmup_runs = 3;
    printf("Warming up (%d runs)...\n", warmup_runs);
    for (int w = 0; w < warmup_runs; w++) {
        (void)energy(bx, by, bz, bvx, bvy, bvz, bmass, n);
        for (int64_t j = 0; j < num_steps / 100; j++) {
            advance(bx, by, bz, bvx, bvy, bvz, bmass, dt, n);
        }
    }

    offset_momentum(bvx, bvy, bvz, bmass, n, solar_mass);

    printf("Running measured iteration...\n");
    double initial_energy = energy(bx, by, bz, bvx, bvy, bvz, bmass, n);

    auto start = std::chrono::high_resolution_clock::now();
    for (int64_t i = 0; i < num_steps; i++) {
        advance(bx, by, bz, bvx, bvy, bvz, bmass, dt, n);
    }
    auto end = std::chrono::high_resolution_clock::now();
    double elapsed = std::chrono::duration<double, std::milli>(end - start).count();

    double final_energy = energy(bx, by, bz, bvx, bvy, bvz, bmass, n);

    printf("Initial energy: %.17g\n", initial_energy);
    printf("Final energy: %.17g\n", final_energy);
    printf("Time: %.6f ms\n", elapsed);
    printf("Steps per second: %.6f million\n", (double)num_steps / (elapsed / 1000.0) / 1e6);
    return 0;
}
