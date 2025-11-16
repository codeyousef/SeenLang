// Simple Recursive Sum Benchmark

fn recursive_sum(n: i64) -> i64 {
    if n <= 0 {
        return 0;
    }
    n + recursive_sum(n - 1)
}

fn main() {
    println!("Benchmark 1: Recursive Sum (Rust)");

    let start = std::time::Instant::now();
    let result = recursive_sum(10000);
    let elapsed = start.elapsed();

    println!("Result: {}", result);
    println!("Time: {} ms", elapsed.as_millis());
}
