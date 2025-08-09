// Reactive Programming Benchmark - Rust Implementation (Simplified)
use std::time::Instant;

struct ReactiveBenchmark;

impl ReactiveBenchmark {
    fn test_imperative(iterations: usize, data_size: usize) -> f64 {
        let start = Instant::now();
        
        for _ in 0..iterations {
            let data: Vec<i32> = (0..data_size as i32).collect();
            
            // Process imperatively
            let mut result = Vec::new();
            for val in data {
                if val % 2 == 0 {
                    result.push(val * 2);
                }
            }
        }
        
        start.elapsed().as_secs_f64()
    }
    
    fn test_simple_reactive(iterations: usize, data_size: usize) -> f64 {
        let start = Instant::now();
        
        for _ in 0..iterations {
            let data: Vec<i32> = (0..data_size as i32).collect();
            
            // Simulate reactive with iterator chains (zero-cost in Rust)
            let _result: Vec<i32> = data
                .into_iter()
                .filter(|val| val % 2 == 0)
                .map(|val| val * 2)
                .collect();
        }
        
        start.elapsed().as_secs_f64()
    }
    
    fn test_complex_composition(iterations: usize, data_size: usize) -> f64 {
        let start = Instant::now();
        
        for _ in 0..iterations {
            let data: Vec<i32> = (0..data_size as i32).collect();
            
            // Complex chain
            let _result: Vec<i32> = data
                .into_iter()
                .filter(|val| *val > 10)
                .map(|val| val * 3)
                .filter(|val| *val < 1000)
                .map(|val| val / 2)
                .collect();
        }
        
        start.elapsed().as_secs_f64()
    }
    
    fn test_backpressure(iterations: usize, data_size: usize) -> f64 {
        let start = Instant::now();
        const BUFFER_LIMIT: usize = 100;
        
        for _ in 0..iterations {
            let data: Vec<i32> = (0..data_size as i32).collect();
            
            // Simulate backpressure with take
            let _result: Vec<i32> = data
                .into_iter()
                .map(|val| val * 2)
                .take(BUFFER_LIMIT)
                .collect();
        }
        
        start.elapsed().as_secs_f64()
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let iterations = args.get(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(1000);
    let data_size = args.get(2)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(1000);
    
    let imperative_time = ReactiveBenchmark::test_imperative(iterations, data_size);
    let simple_reactive_time = ReactiveBenchmark::test_simple_reactive(iterations, data_size);
    let complex_time = ReactiveBenchmark::test_complex_composition(iterations, data_size);
    let backpressure_time = ReactiveBenchmark::test_backpressure(iterations, data_size);
    
    let overhead = ((simple_reactive_time - imperative_time) / imperative_time) * 100.0;
    let zero_cost = overhead.abs() < 5.0;  // Less than 5% overhead
    
    println!("{{");
    println!("  \"language\": \"rust\",");
    println!("  \"benchmark\": \"reactive_zero_cost\",");
    println!("  \"iterations\": {},", iterations);
    println!("  \"data_size\": {},", data_size);
    println!("  \"results\": {{");
    println!("    \"imperative\": {},", imperative_time);
    println!("    \"simple_reactive\": {},", simple_reactive_time);
    println!("    \"complex_composition\": {},", complex_time);
    println!("    \"backpressure\": {},", backpressure_time);
    println!("    \"overhead_percent\": {}", overhead);
    println!("  }},");
    println!("  \"zero_cost\": {}", zero_cost);
    println!("}}");
}