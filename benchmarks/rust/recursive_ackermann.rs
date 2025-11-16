// Ackermann function - deep recursion benchmark
use std::time::Instant;

fn ackermann(m: i64, n: i64) -> i64 {
    if m == 0 {
        n + 1
    } else if n == 0 {
        ackermann(m - 1, 1)
    } else {
        ackermann(m - 1, ackermann(m, n - 1))
    }
}

fn main() {
    let start = Instant::now();
    let result = ackermann(3, 10);
    let elapsed = start.elapsed();

    println!("Ackermann(3, 10) = {}", result);
    println!("Time: {:.3}s", elapsed.as_secs_f64());
}
