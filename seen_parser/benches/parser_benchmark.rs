use criterion::{black_box, criterion_group, criterion_main, Criterion};
use seen_parser::{parse_program, Parser};
use seen_lexer::{Lexer, KeywordManager};

fn benchmark_simple_program(c: &mut Criterion) {
    let source = r#"
    func main() {
        val x = 42;
        println(x);
    }
    "#;
    
    let keyword_manager = KeywordManager::new_for_testing("english");
    let mut lexer = Lexer::new(source, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();
    
    c.bench_function("parse_simple_program",  < /dev/null | b| {
        b.iter(|| {
            parse_program(black_box(tokens.clone()))
        });
    });
}

fn benchmark_complex_program(c: &mut Criterion) {
    let mut source = String::new();
    
    // Generate a program with many functions
    for i in 0..50 {
        source.push_str(&format!(
            "func function_{}(x: Int) -> Int {{ 
                val a = x + {};
                val b = a * 2;
                if b > 100 {{
                    return b - 10;
                }} else {{
                    return b + 10;
                }}
            }}\n",
            i, i
        ));
    }
    source.push_str("func main() { println(function_0(42)); }");
    
    let keyword_manager = KeywordManager::new_for_testing("english");
    let mut lexer = Lexer::new(&source, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();
    
    c.bench_function("parse_complex_program", |b| {
        b.iter(|| {
            parse_program(black_box(tokens.clone()))
        });
    });
}

fn benchmark_deeply_nested(c: &mut Criterion) {
    let mut source = String::from("func nested() { ");
    
    // Create deeply nested if statements
    for _ in 0..20 {
        source.push_str("if true { ");
    }
    source.push_str("val x = 42;");
    for _ in 0..20 {
        source.push_str(" }");
    }
    source.push_str(" }");
    
    let keyword_manager = KeywordManager::new_for_testing("english");
    let mut lexer = Lexer::new(&source, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();
    
    c.bench_function("parse_deeply_nested", |b| {
        b.iter(|| {
            parse_program(black_box(tokens.clone()))
        });
    });
}

criterion_group!(benches, benchmark_simple_program, benchmark_complex_program, benchmark_deeply_nested);
criterion_main!(benches);
