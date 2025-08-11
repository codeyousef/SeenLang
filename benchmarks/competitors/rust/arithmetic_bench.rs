// Rust Arithmetic Microbenchmark Implementation
// Equivalent to Seen's arithmetic operations for fair comparison

use std::time::Instant;

pub struct ArithmeticBenchmark {
    iterations: u32,
    data_size: usize,
}

impl ArithmeticBenchmark {
    pub fn new(iterations: u32, data_size: usize) -> Self {
        ArithmeticBenchmark {
            iterations,
            data_size,
        }
    }

    // 32-bit integer addition benchmark
    pub fn benchmark_i32_addition(&self) -> BenchmarkResult {
        let mut vec_a = Vec::with_capacity(self.data_size);
        let mut vec_b = Vec::with_capacity(self.data_size);
        let mut result_vec = Vec::with_capacity(self.data_size);

        // Initialize test data
        for i in 0..self.data_size {
            vec_a.push(i as i32);
            vec_b.push((i * 2) as i32);
            result_vec.push(0i32);
        }

        let start = Instant::now();

        for _ in 0..self.iterations {
            for i in 0..self.data_size {
                result_vec[i] = vec_a[i] + vec_b[i];
            }
        }

        let elapsed = start.elapsed();
        let total_operations = (self.iterations as i64) * (self.data_size as i64);
        let ops_per_second = total_operations as f64 / elapsed.as_secs_f64();

        BenchmarkResult {
            name: "i32_addition".to_string(),
            language: "rust".to_string(),
            execution_time_ns: elapsed.as_nanos() as i64,
            memory_peak_bytes: (self.data_size * 3 * 4) as i64,
            operations_per_second: ops_per_second,
            success: true,
            error_message: None,
        }
    }

    // 32-bit integer multiplication benchmark
    pub fn benchmark_i32_multiplication(&self) -> BenchmarkResult {
        let mut vec_a = Vec::with_capacity(self.data_size);
        let mut vec_b = Vec::with_capacity(self.data_size);
        let mut result_vec = Vec::with_capacity(self.data_size);

        for i in 0..self.data_size {
            vec_a.push((i % 1000 + 1) as i32);
            vec_b.push((i % 500 + 1) as i32);
            result_vec.push(0i32);
        }

        let start = Instant::now();

        for _ in 0..self.iterations {
            for i in 0..self.data_size {
                result_vec[i] = vec_a[i] * vec_b[i];
            }
        }

        let elapsed = start.elapsed();
        let total_operations = (self.iterations as i64) * (self.data_size as i64);
        let ops_per_second = total_operations as f64 / elapsed.as_secs_f64();

        BenchmarkResult {
            name: "i32_multiplication".to_string(),
            language: "rust".to_string(),
            execution_time_ns: elapsed.as_nanos() as i64,
            memory_peak_bytes: (self.data_size * 3 * 4) as i64,
            operations_per_second: ops_per_second,
            success: true,
            error_message: None,
        }
    }

    // 64-bit floating-point operations benchmark
    pub fn benchmark_f64_operations(&self) -> BenchmarkResult {
        let mut vec_a = Vec::with_capacity(self.data_size);
        let mut vec_b = Vec::with_capacity(self.data_size);
        let mut result_vec = Vec::with_capacity(self.data_size);

        for i in 0..self.data_size {
            vec_a.push(i as f64 * 0.001 + 0.001);
            vec_b.push(i as f64 * 0.002 + 0.002);
            result_vec.push(0.0f64);
        }

        let start = Instant::now();

        for _ in 0..self.iterations {
            for i in 0..self.data_size {
                let intermediate = vec_a[i] + vec_b[i];
                let intermediate2 = intermediate * vec_a[i];
                result_vec[i] = intermediate2 / vec_b[i];
            }
        }

        let elapsed = start.elapsed();
        let total_operations = (self.iterations as i64) * (self.data_size as i64) * 3;
        let ops_per_second = total_operations as f64 / elapsed.as_secs_f64();

        BenchmarkResult {
            name: "f64_mixed_operations".to_string(),
            language: "rust".to_string(),
            execution_time_ns: elapsed.as_nanos() as i64,
            memory_peak_bytes: (self.data_size * 3 * 8) as i64,
            operations_per_second: ops_per_second,
            success: true,
            error_message: None,
        }
    }

    // Bitwise operations benchmark
    pub fn benchmark_bitwise_operations(&self) -> BenchmarkResult {
        let mut vec_a = Vec::with_capacity(self.data_size);
        let mut vec_b = Vec::with_capacity(self.data_size);
        let mut result_vec = Vec::with_capacity(self.data_size);

        for i in 0..self.data_size {
            vec_a.push(i as u32);
            vec_b.push((i as u32).wrapping_mul(0x9E3779B9));
            result_vec.push(0u32);
        }

        let start = Instant::now();

        for _ in 0..self.iterations {
            for i in 0..self.data_size {
                let a = vec_a[i];
                let b = vec_b[i];
                let and_result = a & b;
                let or_result = and_result | a;
                let xor_result = or_result ^ b;
                result_vec[i] = xor_result;
            }
        }

        let elapsed = start.elapsed();
        let total_operations = (self.iterations as i64) * (self.data_size as i64) * 3;
        let ops_per_second = total_operations as f64 / elapsed.as_secs_f64();

        BenchmarkResult {
            name: "bitwise_operations".to_string(),
            language: "rust".to_string(),
            execution_time_ns: elapsed.as_nanos() as i64,
            memory_peak_bytes: (self.data_size * 3 * 4) as i64,
            operations_per_second: ops_per_second,
            success: true,
            error_message: None,
        }
    }

    pub fn run_all(&self) -> Vec<BenchmarkResult> {
        vec![
            self.benchmark_i32_addition(),
            self.benchmark_i32_multiplication(),
            self.benchmark_f64_operations(),
            self.benchmark_bitwise_operations(),
        ]
    }
}

#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub name: String,
    pub language: String,
    pub execution_time_ns: i64,
    pub memory_peak_bytes: i64,
    pub operations_per_second: f64,
    pub success: bool,
    pub error_message: Option<String>,
}

fn main() {
    let benchmark = ArithmeticBenchmark::new(1000, 100000);
    let results = benchmark.run_all();
    
    for result in results {
        println!("{}: {:.2} ops/sec ({:.2}ms)", 
                 result.name, 
                 result.operations_per_second,
                 result.execution_time_ns as f64 / 1_000_000.0);
    }
}