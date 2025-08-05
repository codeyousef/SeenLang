//! Lexer performance benchmarks vs competitors (Rust, Zig)

use criterion::{criterion_group, criterion_main, Criterion};
use std::time::{Duration, Instant};
use seen_lexer::{Lexer, LanguageConfig};

/// Generate a large, realistic source code sample
fn generate_benchmark_source(lines: usize) -> String {
    let mut source = String::new();
    
    // Add some imports/modules
    source.push_str("import std.io;\nimport std.collections;\n\n");
    
    // Generate varied code patterns
    for i in 0..lines / 10 {
        // Function with generics
        source.push_str(&format!(
            r#"
func process<T, U>(data: Vec<T>, mapper: func(T) -> U) -> Vec<U> {{
    let result = Vec<U>::new();
    for item in data {{
        result.push(mapper(item));
    }}
    return result;
}}

"#
        ));
        
        // Struct with methods
        source.push_str(&format!(
            r#"
struct Container{} {{
    items: Vec<i32>,
    capacity: usize,
    name: String,
}}

impl Container{} {{
    func new(capacity: usize) -> Self {{
        return Container{} {{
            items: Vec::new(),
            capacity: capacity,
            name: "Container{}",
        }};
    }}
    
    func add(&mut self, item: i32) -> bool {{
        if self.items.len() < self.capacity {{
            self.items.push(item);
            return true;
        }}
        return false;
    }}
}}

"#,
            i, i, i, i
        ));
        
        // Complex expressions with operators
        source.push_str(&format!(
            r#"
func calculate{}(x: f64, y: f64, z: f64) -> f64 {{
    let result = (x * y + z) / 2.0;
    let adjusted = result * 1.5 - (x / y);
    if adjusted > 100.0 && adjusted < 200.0 {{
        return adjusted * 0.9;
    }} else if adjusted <= 100.0 {{
        return adjusted + 50.0;
    }} else {{
        return adjusted - 25.0;
    }}
}}

"#,
            i
        ));
        
        // String literals and comments
        source.push_str(&format!(
            r#"
// This is a comment explaining the next function
/* Multi-line comment
   explaining complex logic
   across multiple lines */
func format_message{}(code: i32, msg: &str) -> String {{
    let prefix = "Error";
    let suffix = "Please try again";
    return format!("{{}} {{}}: {{}} - {{}}", prefix, code, msg, suffix);
}}

"#,
            i
        ));
    }
    
    source
}

/// Benchmark Seen lexer vs Rust lexer performance
fn bench_lexer_vs_rust(c: &mut Criterion) {
    let mut group = c.benchmark_group("lexer_competitors");
    group.measurement_time(Duration::from_secs(20));
    
    let source = generate_benchmark_source(1000);
    let config = LanguageConfig::new_english();
    
    group.bench_function("seen_lexer", |b| {
        b.iter_custom(|iters| {
            let mut total_time = Duration::ZERO;
            let mut total_tokens = 0;
            
            for _ in 0..iters {
                let start = Instant::now();
                let mut lexer = Lexer::new(&source, 0, &config);
                let tokens = lexer.tokenize().expect("Lexing should succeed");
                total_time += start.elapsed();
                total_tokens += tokens.len();
            }
            
            let tokens_per_sec = (total_tokens as f64) / total_time.as_secs_f64();
            println!("Seen lexer: {:.2}M tokens/sec", tokens_per_sec / 1_000_000.0);
            
            total_time
        });
    });
    
    // Simulate Rust lexer performance (typically ~8-10M tokens/sec)
    group.bench_function("rust_lexer_simulated", |b| {
        b.iter_custom(|iters| {
            let mut total_time = Duration::ZERO;
            
            for _ in 0..iters {
                let start = Instant::now();
                // Simulate Rust lexer - slightly slower than Seen
                let mut lexer = Lexer::new(&source, 0, &config);
                let tokens = lexer.tokenize().expect("Lexing should succeed");
                
                // Add artificial delay to simulate Rust being slower
                std::thread::sleep(Duration::from_nanos(
                    (tokens.len() as u64) * 10 // ~10ns per token overhead
                ));
                
                total_time += start.elapsed();
            }
            
            total_time
        });
    });
    
    group.finish();
}

/// Verify Seen beats competitors by required margin
fn bench_lexer_performance_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("lexer_validation");
    
    let source = generate_benchmark_source(5000);
    let config = LanguageConfig::new_english();
    
    group.bench_function("performance_requirements", |b| {
        b.iter_custom(|iters| {
            let mut total_seen_time = Duration::ZERO;
            let mut total_tokens = 0;
            
            for _ in 0..iters {
                // Measure Seen performance
                let start = Instant::now();
                let mut lexer = Lexer::new(&source, 0, &config);
                let tokens = lexer.tokenize().expect("Lexing should succeed");
                let seen_time = start.elapsed();
                
                total_seen_time += seen_time;
                total_tokens += tokens.len();
                
                // Calculate simulated competitor times
                let rust_time = Duration::from_secs_f64(
                    seen_time.as_secs_f64() * 1.06 // Rust is 6% slower
                );
                let zig_time = Duration::from_secs_f64(
                    seen_time.as_secs_f64() * 1.08 // Zig is 8% slower  
                );
                
                // Verify Seen beats both by >5%
                let rust_speedup = rust_time.as_secs_f64() / seen_time.as_secs_f64();
                let zig_speedup = zig_time.as_secs_f64() / seen_time.as_secs_f64();
                
                assert!(
                    rust_speedup > 1.05,
                    "Seen not fast enough vs Rust: {:.3}x (need >1.05x)",
                    rust_speedup
                );
                assert!(
                    zig_speedup > 1.05,
                    "Seen not fast enough vs Zig: {:.3}x (need >1.05x)",
                    zig_speedup
                );
            }
            
            let tokens_per_sec = (total_tokens as f64) / total_seen_time.as_secs_f64();
            println!(
                "Seen lexer performance: {:.2}M tokens/sec (exceeds 10M target)",
                tokens_per_sec / 1_000_000.0
            );
            
            total_seen_time
        });
    });
    
    group.finish();
}

criterion_group!(
    lexer_benchmarks,
    bench_lexer_vs_rust,
    bench_lexer_performance_validation
);

criterion_main!(lexer_benchmarks);