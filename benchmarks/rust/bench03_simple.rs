// Simple Fibonacci Benchmark

fn fib(n: i64) -> i64 {
    if n <= 1 {
        return n;
    }
    fib(n - 1) + fib(n - 2)
}

fn main() {
    println!("Benchmark 3: Fibonacci (Rust)");

    let start = std::time::Instant::now();
    let result = fib(30);
    let elapsed = start.elapsed();

    println!("fib(30) = {}", result);
    println!("Time: {} ms", elapsed.as_millis());
}
