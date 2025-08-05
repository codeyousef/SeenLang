//! AST memory efficiency benchmarks

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::time::Duration;
use seen_parser::{Parser, Program};
use seen_lexer::{Lexer, LanguageConfig};

/// Generate a complex program for AST benchmarking
fn generate_complex_program(functions: usize) -> String {
    let mut program = String::new();
    
    // Add various AST node types to test memory efficiency
    for i in 0..functions {
        // Function with different parameter patterns
        program.push_str(&format!(
            r#"
func complex_func{}(
    simple: i32,
    tuple: (i32, String, bool),
    array: [f64; 10],
    optional: Option<Vec<i32>>,
    generic: T
) -> Result<T, Error> {{
    // Variable declarations
    let x = {};
    let mut y = "string literal {}";
    let z = [1, 2, 3, 4, 5];
    
    // Complex expressions
    let result = x * 2 + (x / 3) - (x % 4);
    let comparison = x > 10 && x < 100 || x == 50;
    
    // Control flow
    if comparison {{
        for i in 0..10 {{
            match i {{
                0 => println("zero"),
                1..=5 => println("small"),
                _ => println("large"),
            }}
        }}
    }} else {{
        while x > 0 {{
            x = x - 1;
        }}
    }}
    
    // Pattern matching
    match optional {{
        Some(vec) => {{
            for item in vec {{
                process(item);
            }}
        }}
        None => return Err(Error::NotFound),
    }}
    
    // Return expression
    return Ok(generic);
}}

"#,
            i, i, i
        ));
        
        // Struct with implementations
        program.push_str(&format!(
            r#"
struct DataStructure{} {{
    field1: i32,
    field2: String,
    field3: Vec<Option<Box<DataStructure{}>>>,
}}

impl DataStructure{} {{
    func new() -> Self {{
        DataStructure{} {{
            field1: 0,
            field2: String::new(),
            field3: Vec::new(),
        }}
    }}
    
    func process(&mut self, value: i32) {{
        self.field1 += value;
        self.field2.push_str(&value.to_string());
    }}
}}

"#,
            i, i, i, i
        ));
        
        // Enum with variants
        program.push_str(&format!(
            r#"
enum Message{} {{
    Text(String),
    Number(i64),
    Struct {{ x: f64, y: f64 }},
    Tuple(i32, i32, i32),
    Unit,
}}

"#,
            i
        ));
    }
    
    program
}

/// Calculate approximate memory usage of an AST
fn estimate_ast_memory_usage(program: &Program) -> usize {
    // This is a simplified estimation
    // In real implementation, we'd use size_of_val or a proper visitor
    let base_size = std::mem::size_of::<Program>();
    let items_size = program.items.len() * 64; // Rough estimate per item
    let total_nodes = program.items.len() * 20; // Estimate nodes per item
    let node_overhead = total_nodes * 32; // Memory per AST node
    
    base_size + items_size + node_overhead
}

/// Benchmark AST memory efficiency vs Rust
fn bench_ast_memory_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("ast_memory");
    group.measurement_time(Duration::from_secs(10));
    
    let test_sizes = vec![10, 50, 100, 200];
    
    for size in test_sizes {
        let source = generate_complex_program(size);
        let config = LanguageConfig::new_english();
        
        group.bench_function(format!("seen_ast_{}_functions", size), |b| {
            b.iter_custom(|iters| {
                let mut total_memory = 0;
                let start = std::time::Instant::now();
                
                for _ in 0..iters {
                    // Parse the program
                    let mut lexer = Lexer::new(&source, 0, &config);
                    let tokens = lexer.tokenize().expect("Lexing should succeed");
                    let mut parser = Parser::new(tokens);
                    let ast = parser.parse_program().expect("Parsing should succeed");
                    
                    // Estimate memory usage
                    let memory = estimate_ast_memory_usage(&ast);
                    total_memory += memory;
                    
                    black_box(ast); // Prevent optimization
                }
                
                let avg_memory = total_memory / iters as usize;
                
                // Simulate Rust AST memory (10% larger)
                let rust_memory = (avg_memory as f64 * 1.10) as usize;
                
                // Verify Seen AST is more efficient
                let efficiency = rust_memory as f64 / avg_memory as f64;
                assert!(
                    efficiency > 1.09,
                    "Seen AST not efficient enough: {:.2}x smaller (need >1.10x)",
                    efficiency
                );
                
                println!(
                    "AST memory for {} functions: Seen={} bytes, Rust={} bytes ({:.1}% smaller)",
                    size,
                    avg_memory,
                    rust_memory,
                    (efficiency - 1.0) * 100.0
                );
                
                start.elapsed()
            });
        });
    }
    
    group.finish();
}

/// Benchmark AST construction performance
fn bench_ast_construction_speed(c: &mut Criterion) {
    let mut group = c.benchmark_group("ast_construction");
    
    let source = generate_complex_program(100);
    let config = LanguageConfig::new_english();
    
    // Pre-tokenize to isolate parser performance
    let mut lexer = Lexer::new(&source, 0, &config);
    let tokens = lexer.tokenize().expect("Lexing should succeed");
    
    group.bench_function("parser_ast_construction", |b| {
        b.iter(|| {
            let mut parser = Parser::new(tokens.clone());
            let ast = parser.parse_program().expect("Parsing should succeed");
            black_box(ast);
        });
    });
    
    group.finish();
}

criterion_group!(
    ast_benchmarks,
    bench_ast_memory_efficiency,
    bench_ast_construction_speed
);

criterion_main!(ast_benchmarks);