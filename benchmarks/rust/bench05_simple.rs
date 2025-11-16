// Simple Factorial Benchmark

fn factorial(n: i64) -> i64 {
    if n <= 1 {
        return 1;
    }
    n * factorial(n - 1)
}

fn main() {
    println!("Benchmark 5: Factorial (Rust)");

    let start = std::time::Instant::now();
    let result = factorial(20);
    let elapsed = start.elapsed();

    println!("factorial(20) = {}", result);
    println!("Time: {} ms", elapsed.as_millis());
}
