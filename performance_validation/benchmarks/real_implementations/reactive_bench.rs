// Reactive Programming Benchmark - Rust Implementation
use std::time::Instant;
use std::sync::Arc;
use std::sync::Mutex;
use std::collections::VecDeque;

// Simple Observable implementation
struct Observable<T> {
    observers: Vec<Box<dyn Fn(&T)>>,
}

impl<T: Clone + 'static> Observable<T> {
    fn new() -> Self {
        Observable {
            observers: Vec::new(),
        }
    }
    
    fn subscribe<F: Fn(&T) + 'static>(&mut self, observer: F) {
        self.observers.push(Box::new(observer));
    }
    
    fn emit(&self, value: &T) {
        for observer in &self.observers {
            observer(value);
        }
    }
}

struct ReactiveBenchmark;

impl ReactiveBenchmark {
    fn test_imperative(iterations: usize, data_size: usize) -> f64 {
        let start = Instant::now();
        
        for _ in 0..iterations {
            let data: Vec<i32> = (0..data_size as i32).collect();
            
            // Process imperatively
            let _result: Vec<i32> = data
                .iter()
                .filter(|&val| val % 2 == 0)
                .map(|&val| val * 2)
                .collect();
        }
        
        start.elapsed().as_secs_f64()
    }
    
    fn test_simple_reactive(iterations: usize, data_size: usize) -> f64 {
        let start = Instant::now();
        
        for _ in 0..iterations {
            let mut result = Vec::new();
            let result_ptr = Arc::new(std::sync::Mutex::new(&mut result));
            
            // Create reactive chain
            let mut source = Observable::new();
            let result_clone = result_ptr.clone();
            
            source.subscribe(move |val: &i32| {
                if val % 2 == 0 {
                    let mapped = val * 2;
                    if let Ok(mut res) = result_clone.lock() {
                        res.push(mapped);
                    }
                }
            });
            
            // Emit data
            for i in 0..data_size as i32 {
                source.emit(&i);
            }
        }
        
        start.elapsed().as_secs_f64()
    }
    
    fn test_complex_composition(iterations: usize, data_size: usize) -> f64 {
        let start = Instant::now();
        
        for _ in 0..iterations {
            let mut result = Vec::new();
            let result_ptr = Arc::new(std::sync::Mutex::new(&mut result));
            
            let mut source = Observable::new();
            let result_clone = result_ptr.clone();
            
            source.subscribe(move |val: &i32| {
                if *val > 10 {
                    let mapped = val * 3;
                    if mapped < 1000 {
                        let final_val = mapped / 2;
                        if let Ok(mut res) = result_clone.lock() {
                            res.push(final_val);
                        }
                    }
                }
            });
            
            for i in 0..data_size as i32 {
                source.emit(&i);
            }
        }
        
        start.elapsed().as_secs_f64()
    }
    
    fn test_backpressure(iterations: usize, data_size: usize) -> f64 {
        let start = Instant::now();
        
        for _ in 0..iterations {
            let mut buffer = std::collections::VecDeque::new();
            let mut result = Vec::new();
            let buffer_limit = 100;
            
            let mut source = Observable::new();
            let buffer_ptr = Arc::new(std::sync::Mutex::new(&mut buffer));
            let result_ptr = Arc::new(std::sync::Mutex::new(&mut result));
            
            let buffer_clone = buffer_ptr.clone();
            let result_clone = result_ptr.clone();
            
            source.subscribe(move |val: &i32| {
                if let (Ok(mut buf), Ok(mut res)) = (buffer_clone.lock(), result_clone.lock()) {
                    if buf.len() < buffer_limit {
                        buf.push_back(*val);
                    }
                    // Process buffered items
                    while !buf.is_empty() && res.len() < buffer_limit {
                        if let Some(item) = buf.pop_front() {
                            res.push(item * 2);
                        }
                    }
                }
            });
            
            for i in 0..data_size as i32 {
                source.emit(&i);
            }
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