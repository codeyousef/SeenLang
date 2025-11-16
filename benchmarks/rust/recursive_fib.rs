// Fibonacci recursive benchmark
use std::time::Instant;

fn fibonacci(n: i64) -> i64 {
    if n <= 1 {
        n
    } else {
        fibonacci(n - 1) + fibonacci(n - 2)
    }
}

fn main() {
    let start = Instant::now();
    let result = fibonacci(40);
    let elapsed = start.elapsed();

    println!("Fibonacci(40) = {}", result);
    println!("Time: {:.3}s", elapsed.as_secs_f64());
}
