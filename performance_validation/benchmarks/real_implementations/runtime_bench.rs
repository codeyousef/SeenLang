// Runtime Benchmark - Rust Implementation
use std::time::Instant;

struct RuntimeBenchmark;

impl RuntimeBenchmark {
    fn fibonacci(n: u32) -> u64 {
        if n <= 1 {
            n as u64
        } else {
            Self::fibonacci(n - 1) + Self::fibonacci(n - 2)
        }
    }
    
    fn test_runtime(iterations: usize) -> f64 {
        let start = Instant::now();
        
        let mut result = 0.0;
        for _ in 0..iterations {
            // Various runtime operations
            result += Self::fibonacci(20) as f64;
            
            // Math operations
            for j in 0..1000 {
                let j_f = j as f64;
                result += j_f.sin() * j_f.cos();
            }
            
            // String operations
            let mut str = String::from("Hello");
            for _ in 0..100 {
                str.push_str(" World");
            }
            
            // Array operations
            let mut vec = vec![0; 1000];
            for j in 0..1000 {
                vec[j] = j * j;
            }
        }
        
        start.elapsed().as_secs_f64()
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let iterations = args.get(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(30);
    
    let mut times = Vec::new();
    for _ in 0..iterations {
        times.push(RuntimeBenchmark::test_runtime(10));
    }
    
    let sum: f64 = times.iter().sum();
    let mean = sum / times.len() as f64;
    
    println!("{{");
    println!("  \"language\": \"rust\",");
    println!("  \"benchmark\": \"runtime\",");
    println!("  \"iterations\": {},", iterations);
    println!("  \"operations\": 67650,");
    print!("  \"times\": [");
    for (i, time) in times.iter().enumerate() {
        if i > 0 { print!(", "); }
        print!("{}", time);
    }
    println!("],");
    println!("  \"average_time\": {},", mean);
    println!("  \"ops_per_second\": {}", 67650.0 / mean);
    println!("}}");
}