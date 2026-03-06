#!/bin/bash
# Seen vs GCC -O3 Performance Comparison
# Uses the self-hosted compiler (bootstrap/stage1_frozen or compiler_seen/target/seen)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
SEEN="$ROOT_DIR/compiler_seen/target/seen"
RESULTS="$SCRIPT_DIR/seen_vs_gcc_results.txt"

if [ ! -f "$SEEN" ]; then
    SEEN="$ROOT_DIR/bootstrap/stage1_frozen"
fi

if [ ! -f "$SEEN" ]; then
    echo "Error: No Seen compiler found. Run: ./scripts/safe_rebuild.sh"
    exit 1
fi

echo "================================================================"
echo "  Seen vs GCC -O3 Performance Comparison"
echo "  Compiler: $SEEN"
echo "================================================================"
echo ""

echo "# Seen vs GCC -O3 Results ($(date))" > "$RESULTS"
echo "" >> "$RESULTS"

run_benchmark() {
    local name="$1"
    local seen_src="$2"
    local c_src="$3"
    local iterations="${4:-3}"

    echo "=== $name ==="

    # Compile Seen
    rm -rf "$ROOT_DIR/.seen_cache/"
    if ! "$SEEN" compile "$seen_src" /tmp/seen_bench 2>/dev/null; then
        echo "  [SKIP] Seen compilation failed"
        echo "| $name | SKIP | - | - |" >> "$RESULTS"
        return
    fi

    # Compile C with GCC -O3
    if ! gcc -O3 -march=native -o /tmp/gcc_bench "$c_src" -lm 2>/dev/null; then
        echo "  [SKIP] GCC compilation failed"
        echo "| $name | - | SKIP | - |" >> "$RESULTS"
        return
    fi

    # Measure Seen compilation time
    local seen_compile_start=$(date +%s%N)
    rm -rf "$ROOT_DIR/.seen_cache/"
    "$SEEN" compile "$seen_src" /tmp/seen_bench 2>/dev/null
    local seen_compile_end=$(date +%s%N)
    local seen_compile_ms=$(( (seen_compile_end - seen_compile_start) / 1000000 ))

    # Measure GCC compilation time
    local gcc_compile_start=$(date +%s%N)
    gcc -O3 -march=native -o /tmp/gcc_bench "$c_src" -lm 2>/dev/null
    local gcc_compile_end=$(date +%s%N)
    local gcc_compile_ms=$(( (gcc_compile_end - gcc_compile_start) / 1000000 ))

    # Run benchmarks
    local seen_total=0
    local gcc_total=0

    for i in $(seq 1 $iterations); do
        local s_start=$(date +%s%N)
        /tmp/seen_bench > /dev/null 2>&1 || true
        local s_end=$(date +%s%N)
        seen_total=$(( seen_total + (s_end - s_start) ))

        local g_start=$(date +%s%N)
        /tmp/gcc_bench > /dev/null 2>&1 || true
        local g_end=$(date +%s%N)
        gcc_total=$(( gcc_total + (g_end - g_start) ))
    done

    local seen_ms=$(( seen_total / iterations / 1000000 ))
    local gcc_ms=$(( gcc_total / iterations / 1000000 ))

    local ratio="N/A"
    if [ "$gcc_ms" -gt 0 ]; then
        # ratio = seen_ms * 100 / gcc_ms (as percentage)
        local pct=$(( seen_ms * 100 / gcc_ms ))
        ratio="${pct}%"
    fi

    echo "  Seen: ${seen_ms}ms (compile: ${seen_compile_ms}ms)"
    echo "  GCC:  ${gcc_ms}ms (compile: ${gcc_compile_ms}ms)"
    echo "  Ratio: $ratio of GCC"
    echo "  Compile speedup: $(( gcc_compile_ms * 10 / (seen_compile_ms + 1) ))x"
    echo ""

    echo "| $name | ${seen_ms}ms | ${gcc_ms}ms | $ratio | ${seen_compile_ms}ms vs ${gcc_compile_ms}ms |" >> "$RESULTS"
}

echo "| Benchmark | Seen | GCC -O3 | Ratio | Compile Time |" >> "$RESULTS"
echo "|-----------|------|---------|-------|-------------|" >> "$RESULTS"

# Create C equivalents for production benchmarks

# 1. Matrix Multiply
cat > /tmp/bench_matrix.c << 'CEOF'
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#define N 500
double a[N][N], b[N][N], c[N][N];
int main() {
    srand(42);
    for (int i = 0; i < N; i++)
        for (int j = 0; j < N; j++) {
            a[i][j] = (double)(rand() % 100) / 10.0;
            b[i][j] = (double)(rand() % 100) / 10.0;
        }
    for (int i = 0; i < N; i++)
        for (int j = 0; j < N; j++) {
            double sum = 0.0;
            for (int k = 0; k < N; k++)
                sum += a[i][k] * b[k][j];
            c[i][j] = sum;
        }
    printf("c[0][0] = %f\n", c[0][0]);
    return 0;
}
CEOF

# 2. Sieve of Eratosthenes
cat > /tmp/bench_sieve.c << 'CEOF'
#include <stdio.h>
#include <string.h>
#define LIMIT 10000000
char sieve[LIMIT + 1];
int main() {
    memset(sieve, 1, sizeof(sieve));
    sieve[0] = sieve[1] = 0;
    for (int i = 2; i * i <= LIMIT; i++)
        if (sieve[i])
            for (int j = i * i; j <= LIMIT; j += i)
                sieve[j] = 0;
    int count = 0;
    for (int i = 0; i <= LIMIT; i++)
        if (sieve[i]) count++;
    printf("Primes up to %d: %d\n", LIMIT, count);
    return 0;
}
CEOF

# 3. N-body simulation
cat > /tmp/bench_nbody.c << 'CEOF'
#include <stdio.h>
#include <math.h>
#define PI 3.141592653589793
#define SOLAR_MASS (4.0 * PI * PI)
#define DAYS_PER_YEAR 365.24
#define N_BODIES 5
typedef struct { double x, y, z, vx, vy, vz, mass; } Body;
Body bodies[N_BODIES];
void init() {
    bodies[0] = (Body){0, 0, 0, 0, 0, 0, SOLAR_MASS};
    bodies[1] = (Body){4.84143144246472090e+00,-1.16032004402742839e+00,-1.03622044471123109e-01,
        1.66007664274403694e-03*DAYS_PER_YEAR,7.69901118419740425e-03*DAYS_PER_YEAR,-6.90460016972063023e-05*DAYS_PER_YEAR,
        9.54791938424326609e-04*SOLAR_MASS};
    bodies[2] = (Body){8.34336671824457987e+00,4.12479856412430479e+00,-4.03523417114321381e-01,
        -2.76742510726862411e-03*DAYS_PER_YEAR,4.99852801234917238e-03*DAYS_PER_YEAR,2.30417297573763929e-05*DAYS_PER_YEAR,
        2.85885980666130812e-04*SOLAR_MASS};
    bodies[3] = (Body){1.28943695621391310e+01,-1.51111514016986312e+01,-2.23307578892655734e-01,
        2.96460137564761618e-03*DAYS_PER_YEAR,2.37847173959480950e-03*DAYS_PER_YEAR,-2.96589568540237556e-05*DAYS_PER_YEAR,
        4.36624404335156298e-05*SOLAR_MASS};
    bodies[4] = (Body){1.53796971148509165e+01,-2.59193146099879641e+01,1.79258772950371181e-01,
        2.68067772490389322e-03*DAYS_PER_YEAR,1.62824170038242295e-03*DAYS_PER_YEAR,-9.51592254519715870e-05*DAYS_PER_YEAR,
        5.15138902046611451e-05*SOLAR_MASS};
}
double energy() {
    double e = 0;
    for (int i = 0; i < N_BODIES; i++) {
        e += 0.5 * bodies[i].mass * (bodies[i].vx*bodies[i].vx + bodies[i].vy*bodies[i].vy + bodies[i].vz*bodies[i].vz);
        for (int j = i+1; j < N_BODIES; j++) {
            double dx = bodies[i].x - bodies[j].x, dy = bodies[i].y - bodies[j].y, dz = bodies[i].z - bodies[j].z;
            e -= bodies[i].mass * bodies[j].mass / sqrt(dx*dx + dy*dy + dz*dz);
        }
    }
    return e;
}
void advance(double dt) {
    for (int i = 0; i < N_BODIES; i++)
        for (int j = i+1; j < N_BODIES; j++) {
            double dx = bodies[i].x-bodies[j].x, dy = bodies[i].y-bodies[j].y, dz = bodies[i].z-bodies[j].z;
            double d2 = dx*dx+dy*dy+dz*dz;
            double mag = dt / (d2 * sqrt(d2));
            bodies[i].vx -= dx*bodies[j].mass*mag; bodies[i].vy -= dy*bodies[j].mass*mag; bodies[i].vz -= dz*bodies[j].mass*mag;
            bodies[j].vx += dx*bodies[i].mass*mag; bodies[j].vy += dy*bodies[i].mass*mag; bodies[j].vz += dz*bodies[i].mass*mag;
        }
    for (int i = 0; i < N_BODIES; i++) {
        bodies[i].x += dt*bodies[i].vx; bodies[i].y += dt*bodies[i].vy; bodies[i].z += dt*bodies[i].vz;
    }
}
int main() {
    init();
    printf("%.9f\n", energy());
    for (int i = 0; i < 50000000; i++) advance(0.01);
    printf("%.9f\n", energy());
    return 0;
}
CEOF

# Run benchmarks using production .seen files
if [ -f "$SCRIPT_DIR/production/01_matrix_mult.seen" ]; then
    run_benchmark "Matrix Multiply" "$SCRIPT_DIR/production/01_matrix_mult.seen" "/tmp/bench_matrix.c"
fi

if [ -f "$SCRIPT_DIR/production/02_sieve.seen" ]; then
    run_benchmark "Sieve" "$SCRIPT_DIR/production/02_sieve.seen" "/tmp/bench_sieve.c"
fi

if [ -f "$SCRIPT_DIR/production/05_nbody.seen" ]; then
    run_benchmark "N-Body" "$SCRIPT_DIR/production/05_nbody.seen" "/tmp/bench_nbody.c"
fi

# Compilation speed comparison
echo "=== Compilation Speed ==="
echo "" >> "$RESULTS"
echo "## Compilation Speed" >> "$RESULTS"

# Measure Seen compilation speed on the compiler itself
if [ -f "$ROOT_DIR/compiler_seen/src/main_compiler.seen" ]; then
    rm -rf "$ROOT_DIR/.seen_cache/"
    local_start=$(date +%s%N)
    "$SEEN" compile "$ROOT_DIR/compiler_seen/src/main_compiler.seen" /tmp/seen_speed_test 2>/dev/null || true
    local_end=$(date +%s%N)
    seen_self_ms=$(( (local_end - local_start) / 1000000 ))
    echo "  Self-compilation: ${seen_self_ms}ms"
    echo "Self-compilation: ${seen_self_ms}ms" >> "$RESULTS"
fi

echo ""
echo "================================================================"
echo "  Results saved to: $RESULTS"
echo "================================================================"

# Cleanup
rm -f /tmp/seen_bench /tmp/gcc_bench /tmp/seen_speed_test
rm -f /tmp/bench_matrix.c /tmp/bench_sieve.c /tmp/bench_nbody.c
