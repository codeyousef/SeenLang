// Memory Benchmark - Rust Implementation
use std::time::Instant;
use std::collections::HashMap;

struct MemoryBenchmark {
    allocations: Vec<Vec<u8>>,
}

impl MemoryBenchmark {
    fn new() -> Self {
        MemoryBenchmark {
            allocations: Vec::new(),
        }
    }
    
    fn test_allocations(&mut self, size: usize, count: usize) -> f64 {
        let start = Instant::now();
        
        // Allocate
        for i in 0..count {
            let mut vec = vec![0u8; size];
            // Touch memory to ensure it's actually allocated
            for j in 0..size.min(100) {
                vec[j] = (i * j) as u8;
            }
            self.allocations.push(vec);
        }
        
        // Deallocate
        self.allocations.clear();
        
        start.elapsed().as_secs_f64()
    }
    
    fn test_fragmentation(&mut self) -> f64 {
        let start = Instant::now();
        
        // Create fragmented memory pattern
        for i in 0..1000 {
            let size = ((i % 10) + 1) * 1024;
            let vec = vec![0u8; size];
            self.allocations.push(vec);
            
            // Randomly deallocate some
            if i % 3 == 0 && !self.allocations.is_empty() {
                self.allocations.pop();
            }
        }
        
        self.allocations.clear();
        start.elapsed().as_secs_f64()
    }
    
    fn test_collections(&mut self) -> f64 {
        let start = Instant::now();
        
        let mut map = HashMap::new();
        for i in 0..10000 {
            map.insert(i, vec![i as u8; 100]);
        }
        
        let mut vec = Vec::with_capacity(10000);
        for i in 0..10000 {
            vec.push(i * i);
        }
        
        start.elapsed().as_secs_f64()
    }
    
    fn benchmark(&mut self, operations: usize) -> f64 {
        let start = Instant::now();
        
        for _ in 0..operations {
            self.test_allocations(1024, 100);
            self.test_fragmentation();
            self.test_collections();
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
        let mut bench = MemoryBenchmark::new();
        let time = bench.benchmark(1);
        times.push(time);
    }
    
    let sum: f64 = times.iter().sum();
    let mean = sum / times.len() as f64;
    
    println!("{{");
    println!("  \"language\": \"rust\",");
    println!("  \"benchmark\": \"memory\",");
    println!("  \"iterations\": {},", iterations);
    println!("  \"allocations\": 21100,");
    print!("  \"times\": [");
    for (i, time) in times.iter().enumerate() {
        if i > 0 { print!(", "); }
        print!("{}", time);
    }
    println!("],");
    println!("  \"average_time\": {},", mean);
    println!("  \"allocations_per_second\": {}", 21100.0 / mean);
    println!("}}");
}