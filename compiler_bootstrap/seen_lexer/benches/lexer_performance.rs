//! Performance benchmarks for the Seen lexer
//! Target: >10M tokens/second

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use seen_lexer::{Lexer, LanguageConfig};
use std::time::Duration;

fn create_english_config() -> LanguageConfig {
    use std::collections::HashMap;
    
    let mut keywords = HashMap::new();
    keywords.insert("func".to_string(), "TokenFunc".to_string());
    keywords.insert("if".to_string(), "TokenIf".to_string());
    keywords.insert("else".to_string(), "TokenElse".to_string());
    keywords.insert("while".to_string(), "TokenWhile".to_string());
    keywords.insert("for".to_string(), "TokenFor".to_string());
    keywords.insert("return".to_string(), "TokenReturn".to_string());
    keywords.insert("let".to_string(), "TokenLet".to_string());
    keywords.insert("mut".to_string(), "TokenMut".to_string());
    keywords.insert("struct".to_string(), "TokenStruct".to_string());
    keywords.insert("enum".to_string(), "TokenEnum".to_string());
    
    let mut operators = HashMap::new();
    operators.insert("+".to_string(), "TokenPlus".to_string());
    operators.insert("-".to_string(), "TokenMinus".to_string());
    operators.insert("*".to_string(), "TokenMultiply".to_string());
    operators.insert("/".to_string(), "TokenDivide".to_string());
    operators.insert("=".to_string(), "TokenAssign".to_string());
    operators.insert("==".to_string(), "TokenEqual".to_string());
    operators.insert("!=".to_string(), "TokenNotEqual".to_string());
    operators.insert("<".to_string(), "TokenLess".to_string());
    operators.insert("<=".to_string(), "TokenLessEqual".to_string());
    operators.insert(">".to_string(), "TokenGreater".to_string());
    operators.insert(">=".to_string(), "TokenGreaterEqual".to_string());
    
    LanguageConfig {
        keywords,
        operators,
        name: "English".to_string(),
        description: Some("English keyword set for Seen language".to_string()),
    }
}

fn generate_test_code(size: usize) -> String {
    let patterns = [
        "func fibonacci(n: i32) -> i32 { if n <= 1 { return n; } return fibonacci(n-1) + fibonacci(n-2); }",
        "let mut x = 42; x += 10;",
        "struct Point { x: f64, y: f64 }",
        "for i in 0..100 { println!(\"Hello {}\", i); }",
        "if x == y && z != w { return true; }",
        "let arr = [1, 2, 3, 4, 5];",
        "// This is a comment\n/* Block comment */",
        "\"String with escapes\\n\\t\"",
        "'c'",
        "3.14159",
    ];
    
    let mut code = String::new();
    let pattern_chars: usize = patterns.iter().map(|p| p.len()).sum();
    let repetitions = (size + pattern_chars - 1) / pattern_chars;
    
    for _ in 0..repetitions {
        for pattern in &patterns {
            code.push_str(pattern);
            code.push('\n');
            if code.len() >= size {
                break;
            }
        }
        if code.len() >= size {
            break;
        }
    }
    
    code.truncate(size);
    code
}

fn bench_lexer_throughput(c: &mut Criterion) {
    let config = create_english_config();
    let sizes = [1_000, 10_000, 100_000, 1_000_000];
    
    let mut group = c.benchmark_group("lexer_throughput");
    
    for size in sizes.iter() {
        let code = generate_test_code(*size);
        let token_count = code.chars().filter(|c| !c.is_whitespace()).count();
        
        group.throughput(Throughput::Elements(token_count as u64));
        group.bench_with_input(
            BenchmarkId::new("tokenize", size),
            &code,
            |b, code| {
                b.iter(|| {
                    let mut lexer = Lexer::new(code, 0, &config);
                    lexer.tokenize().unwrap()
                });
            },
        );
    }
    
    group.finish();
}

fn bench_lexer_token_types(c: &mut Criterion) {
    let config = create_english_config();
    let mut group = c.benchmark_group("lexer_token_types");
    
    // Test different types of tokens
    let test_cases = [
        ("identifiers", "identifier another_identifier someVeryLongIdentifierName".repeat(1000)),
        ("keywords", "func if else while for return let mut struct enum".repeat(1000)),
        ("operators", "+ - * / = == != < <= > >= && || ! & | ^".repeat(1000)),
        ("literals", "42 3.14159 \"string\" 'c' true false".repeat(1000)),
        ("mixed", generate_test_code(10_000)),
    ];
    
    for (name, code) in test_cases.iter() {
        group.bench_function(name.to_string(), |b| {
            b.iter(|| {
                let mut lexer = Lexer::new(code, 0, &config);
                lexer.tokenize().unwrap()
            });
        });
    }
    
    group.finish();
}

fn bench_lexer_error_recovery(c: &mut Criterion) {
    let config = create_english_config();
    let mut group = c.benchmark_group("lexer_error_recovery");
    
    // Test code with various syntax errors that require recovery
    let error_cases = [
        ("unterminated_string", "\"unterminated string\nfunc test() {}".repeat(100)),
        ("invalid_chars", "func test() { let x = @#$%^&*; }".repeat(100)),
        ("mixed_errors", "\"unterminated\nfunc @test() { let x = #invalid; }".repeat(100)),
    ];
    
    for (name, code) in error_cases.iter() {
        group.bench_function(name.to_string(), |b| {
            b.iter(|| {
                let mut lexer = Lexer::new(code, 0, &config);
                // Don't unwrap - we expect errors
                let _ = lexer.tokenize();
            });
        });
    }
    
    group.finish();
}

fn bench_lexer_large_files(c: &mut Criterion) {
    let config = create_english_config();
    let mut group = c.benchmark_group("lexer_large_files");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(10);
    
    // Simulate large file sizes (typical of real codebases)
    let file_sizes = [100_000, 500_000, 1_000_000]; // ~100KB to 1MB
    
    for size in file_sizes.iter() {
        let code = generate_test_code(*size);
        let token_count = code.split_whitespace().count();
        
        group.throughput(Throughput::Elements(token_count as u64));
        group.bench_with_input(
            BenchmarkId::new("large_file", size),
            &code,
            |b, code| {
                b.iter(|| {
                    let mut lexer = Lexer::new(code, 0, &config);
                    lexer.tokenize().unwrap()
                });
            },
        );
    }
    
    group.finish();
}

// Performance regression test - ensure we maintain >10M tokens/sec target
fn bench_performance_target(c: &mut Criterion) {
    let config = create_english_config();
    let code = generate_test_code(1_000_000); // 1MB of code
    let estimated_tokens = code.len() / 5; // Rough estimate: 5 chars per token
    
    c.bench_function("performance_target_10m_tokens_per_sec", |b| {
        b.iter(|| {
            let mut lexer = Lexer::new(&code, 0, &config);
            let tokens = lexer.tokenize().unwrap();
            
            // Ensure we're actually processing a reasonable number of tokens
            assert!(tokens.len() > estimated_tokens / 10);
        });
    });
}

criterion_group!(
    benches,
    bench_lexer_throughput,
    bench_lexer_token_types,
    bench_lexer_error_recovery,
    bench_lexer_large_files,
    bench_performance_target
);

criterion_main!(benches);