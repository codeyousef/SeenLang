//! Parser performance benchmarks - TARGET: >1M lines/second

use criterion::{criterion_group, criterion_main, Criterion};
use seen_parser::Parser;
use seen_lexer::{Lexer, LanguageConfig};
use std::collections::HashMap;

fn benchmark_parser_performance(c: &mut Criterion) {
    let config = create_test_config();
    let test_program = generate_test_program(1000);
    
    c.bench_function("parser_1k_lines", |b| {
        b.iter(|| {
            let mut lexer = Lexer::new(&test_program, 0, &config);
            let tokens = lexer.tokenize().expect("Tokenization must succeed");
            let mut parser = Parser::new(tokens);
            let _ast = parser.parse_program().expect("Parsing must succeed");
        });
    });
}

fn create_test_config() -> LanguageConfig {
    let mut keywords = HashMap::new();
    keywords.insert("func".to_string(), "TokenFunc".to_string());
    keywords.insert("struct".to_string(), "TokenStruct".to_string());
    keywords.insert("let".to_string(), "TokenLet".to_string());
    keywords.insert("return".to_string(), "TokenReturn".to_string());
    
    let operators = HashMap::new();
    
    LanguageConfig {
        keywords,
        operators,
        name: "English".to_string(),
        description: Some("Test configuration".to_string()),
    }
}

fn generate_test_program(lines: usize) -> String {
    let mut program = String::new();
    for i in 0..lines {
        program.push_str(&format!(
            "func test_function_{}() {{\n    let x = {};\n    return x;\n}}\n\n",
            i, i
        ));
    }
    program
}

criterion_group!(benches, benchmark_parser_performance);
criterion_main!(benches);