// Simple Ackermann Benchmark

fn ackermann(m: i64, n: i64) -> i64 {
    if m == 0 {
        return n + 1;
    }
    if n == 0 {
        return ackermann(m - 1, 1);
    }
    ackermann(m - 1, ackermann(m, n - 1))
}

fn main() {
    println!("Benchmark 4: Ackermann (Rust)");

    let start = std::time::Instant::now();
    let result = ackermann(3, 8);
    let elapsed = start.elapsed();

    println!("ackermann(3, 8) = {}", result);
    println!("Time: {} ms", elapsed.as_millis());
}
