//! Performance benchmarks for regex engine
//!
//! Verifies regex performance targets for lexer and parser workloads

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use seen_std::regex::Regex;

fn bench_regex_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("regex_compilation");
    
    let patterns = vec![
        // Simple patterns
        ("literal", "hello"),
        ("dot", "a.c"),
        ("star", "a*"),
        ("plus", "a+"),
        ("question", "a?"),
        
        // Character classes
        ("digit", r"\d+"),
        ("word", r"\w+"),
        ("whitespace", r"\s+"),
        ("alpha", "[a-zA-Z]+"),
        ("alphanum", "[a-zA-Z0-9]+"),
        
        // Complex patterns for compiler workloads
        ("identifier", r"[a-zA-Z_][a-zA-Z0-9_]*"),
        ("number", r"\d+(\.\d+)?([eE][+-]?\d+)?"),
        ("string_literal", r#""([^"\\]|\\.)*""#),
        ("comment", r"//.*$"),
        ("function", r"fn\s+[a-zA-Z_][a-zA-Z0-9_]*\s*\("),
        
        // Very complex patterns
        ("email", r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}"),
        ("url", r"https?://[a-zA-Z0-9.-]+(/[a-zA-Z0-9._~:/?#[\]@!$&'()*+,;=-]*)?"),
    ];
    
    for (name, pattern) in patterns {
        group.bench_function(name, |b| {
            b.iter(|| {
                let result = Regex::new(black_box(pattern));
                black_box(result);
            });
        });
    }
    
    group.finish();
}

fn bench_regex_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("regex_matching");
    
    // Prepare test data
    let source_code = r#"
    fn main() {
        let message = "Hello, World!";
        println!("{}", message);
        
        let numbers = vec![1, 2, 3, 4, 5];
        for (index, value) in numbers.iter().enumerate() {
            println!("Index: {}, Value: {}", index, value);
        }
        
        // This is a comment
        let result = calculate_sum(10, 20);
        assert_eq!(result, 30);
    }
    
    fn calculate_sum(a: i32, b: i32) -> i32 {
        a + b
    }
    "#;
    
    let large_text = source_code.repeat(100);
    
    // Compile patterns once
    let identifier_regex = Regex::new(r"[a-zA-Z_][a-zA-Z0-9_]*").unwrap();
    let number_regex = Regex::new(r"\d+").unwrap();
    let string_regex = Regex::new(r#""[^"]*""#).unwrap();
    let comment_regex = Regex::new(r"//.*").unwrap();
    let keyword_regex = Regex::new(r"\b(fn|let|for|if|else|while|return)\b").unwrap();
    
    group.bench_function("identifier_matching", |b| {
        b.iter(|| {
            let matches = identifier_regex.find_all(black_box(&large_text));
            black_box(matches);
        });
    });
    
    group.bench_function("number_matching", |b| {
        b.iter(|| {
            let matches = number_regex.find_all(black_box(&large_text));
            black_box(matches);
        });
    });
    
    group.bench_function("string_matching", |b| {
        b.iter(|| {
            let matches = string_regex.find_all(black_box(&large_text));
            black_box(matches);
        });
    });
    
    group.bench_function("comment_matching", |b| {
        b.iter(|| {
            let matches = comment_regex.find_all(black_box(&large_text));
            black_box(matches);
        });
    });
    
    group.bench_function("keyword_matching", |b| {
        b.iter(|| {
            let matches = keyword_regex.find_all(black_box(&large_text));
            black_box(matches);
        });
    });
    
    group.finish();
}

fn bench_regex_lexer_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("regex_lexer_simulation");
    
    // Simulate a simple lexer using multiple regex patterns
    let source_code = r#"
    // Seen language example
    fn fibonacci(n: Int) -> Int {
        if n <= 1 {
            return n;
        }
        return fibonacci(n - 1) + fibonacci(n - 2);
    }
    
    fn main() {
        let numbers = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        for num in numbers {
            let result = fibonacci(num);
            println("fibonacci({}) = {}", num, result);
        }
    }
    "#.repeat(20);
    
    // Lexer patterns
    let patterns = vec![
        ("whitespace", Regex::new(r"\s+").unwrap()),
        ("comment", Regex::new(r"//.*").unwrap()),
        ("identifier", Regex::new(r"[a-zA-Z_][a-zA-Z0-9_]*").unwrap()),
        ("number", Regex::new(r"\d+").unwrap()),
        ("string", Regex::new(r#""[^"]*""#).unwrap()),
        ("operator", Regex::new(r"[+\-*/=<>!&|]").unwrap()),
        ("delimiter", Regex::new(r"[(){}[\],;]").unwrap()),
    ];
    
    group.bench_function("full_lexer_pass", |b| {
        b.iter(|| {
            let mut token_count = 0;
            for (_name, regex) in &patterns {
                let matches = regex.find_all(black_box(&source_code));
                token_count += matches.len();
            }
            black_box(token_count);
        });
    });
    
    group.finish();
}

fn bench_regex_replace(c: &mut Criterion) {
    let mut group = c.benchmark_group("regex_replace");
    
    let text = "This is a test with multiple words and numbers like 123 and 456.".repeat(100);
    
    let number_regex = Regex::new(r"\d+").unwrap();
    let word_regex = Regex::new(r"\b\w+\b").unwrap();
    
    group.bench_function("replace_numbers", |b| {
        b.iter(|| {
            let result = number_regex.replace_all(black_box(&text), "[NUM]");
            black_box(result);
        });
    });
    
    group.bench_function("replace_words", |b| {
        b.iter(|| {
            let result = word_regex.replace_all(black_box(&text), "[WORD]");
            black_box(result);
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_regex_compilation,
    bench_regex_matching,
    bench_regex_lexer_simulation,
    bench_regex_replace
);
criterion_main!(benches);