//! Performance benchmarks for string types
//!
//! Tests SSO (Small String Optimization) and general string performance

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use seen_std::string::{String, StringBuilder};

fn bench_string_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_creation");
    
    // Small strings (SSO)
    group.bench_function("seen_small", |b| {
        b.iter(|| {
            let s = String::from(black_box("Hello"));
            black_box(s);
        });
    });
    
    group.bench_function("std_small", |b| {
        b.iter(|| {
            let s = std::string::String::from(black_box("Hello"));
            black_box(s);
        });
    });
    
    // Large strings (heap)
    let large = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor.";
    group.bench_function("seen_large", |b| {
        b.iter(|| {
            let s = String::from(black_box(large));
            black_box(s);
        });
    });
    
    group.bench_function("std_large", |b| {
        b.iter(|| {
            let s = std::string::String::from(black_box(large));
            black_box(s);
        });
    });
    
    group.finish();
}

fn bench_string_push(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_push");
    
    for size in &[10, 100, 1000] {
        group.bench_with_input(BenchmarkId::new("seen", size), size, |b, &size| {
            b.iter(|| {
                let mut s = String::new();
                for _ in 0..size {
                    s.push(black_box('a'));
                }
                black_box(s);
            });
        });
        
        group.bench_with_input(BenchmarkId::new("std", size), size, |b, &size| {
            b.iter(|| {
                let mut s = std::string::String::new();
                for _ in 0..size {
                    s.push(black_box('a'));
                }
                black_box(s);
            });
        });
    }
    
    group.finish();
}

fn bench_string_concatenation(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_concatenation");
    
    let strings = vec!["Hello", " ", "World", "!", " ", "How", " ", "are", " ", "you", "?"];
    
    group.bench_function("seen_concat", |b| {
        b.iter(|| {
            let mut s = String::new();
            for str in &strings {
                s.push_str(black_box(str));
            }
            black_box(s);
        });
    });
    
    group.bench_function("std_concat", |b| {
        b.iter(|| {
            let mut s = std::string::String::new();
            for str in &strings {
                s.push_str(black_box(str));
            }
            black_box(s);
        });
    });
    
    group.finish();
}

fn bench_string_builder(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_builder");
    
    group.bench_function("seen_builder", |b| {
        b.iter(|| {
            let mut builder = StringBuilder::new();
            for i in 0..100 {
                builder.append("Item ");
                builder.append_format("", i);
                builder.append(", ");
            }
            let s = builder.build();
            black_box(s);
        });
    });
    
    group.bench_function("std_format", |b| {
        b.iter(|| {
            let mut s = std::string::String::new();
            for i in 0..100 {
                s.push_str("Item ");
                s.push_str(&i.to_string());
                s.push_str(", ");
            }
            black_box(s);
        });
    });
    
    group.finish();
}

fn bench_string_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_search");
    
    let haystack = "The quick brown fox jumps over the lazy dog. The quick brown fox jumps over the lazy dog.";
    
    group.bench_function("seen_contains", |b| {
        let s = String::from(haystack);
        b.iter(|| {
            let mut count = 0;
            if s.contains(black_box("fox")) {
                count += 1;
            }
            if s.contains(black_box("cat")) {
                count += 1;
            }
            black_box(count);
        });
    });
    
    group.bench_function("std_contains", |b| {
        let s = std::string::String::from(haystack);
        b.iter(|| {
            let mut count = 0;
            if s.contains(black_box("fox")) {
                count += 1;
            }
            if s.contains(black_box("cat")) {
                count += 1;
            }
            black_box(count);
        });
    });
    
    group.finish();
}

fn bench_string_manipulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_manipulation");
    
    group.bench_function("seen_replace", |b| {
        let s = String::from("Hello World Hello World");
        b.iter(|| {
            let s2 = s.replace(black_box("World"), black_box("Rust"));
            black_box(s2);
        });
    });
    
    group.bench_function("std_replace", |b| {
        let s = std::string::String::from("Hello World Hello World");
        b.iter(|| {
            let s2 = s.replace(black_box("World"), black_box("Rust"));
            black_box(s2);
        });
    });
    
    group.bench_function("seen_case", |b| {
        let s = String::from("Hello World!");
        b.iter(|| {
            let lower = s.to_lowercase();
            let upper = s.to_uppercase();
            black_box((lower, upper));
        });
    });
    
    group.bench_function("std_case", |b| {
        let s = std::string::String::from("Hello World!");
        b.iter(|| {
            let lower = s.to_lowercase();
            let upper = s.to_uppercase();
            black_box((lower, upper));
        });
    });
    
    group.finish();
}

fn bench_string_utf8(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_utf8");
    
    let utf8_str = "Hello 世界 🦀 Здравствуй мир";
    
    group.bench_function("seen_char_count", |b| {
        let s = String::from(utf8_str);
        b.iter(|| {
            let count = s.char_count();
            black_box(count);
        });
    });
    
    group.bench_function("std_char_count", |b| {
        let s = std::string::String::from(utf8_str);
        b.iter(|| {
            let count = s.chars().count();
            black_box(count);
        });
    });
    
    group.bench_function("seen_char_iteration", |b| {
        let s = String::from(utf8_str);
        b.iter(|| {
            let mut count = 0;
            for ch in s.chars() {
                if ch.is_alphabetic() {
                    count += 1;
                }
            }
            black_box(count);
        });
    });
    
    group.bench_function("std_char_iteration", |b| {
        let s = std::string::String::from(utf8_str);
        b.iter(|| {
            let mut count = 0;
            for ch in s.chars() {
                if ch.is_alphabetic() {
                    count += 1;
                }
            }
            black_box(count);
        });
    });
    
    group.finish();
}

fn bench_sso_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("sso_performance");
    
    // Test transition between SSO and heap
    group.bench_function("seen_sso_transition", |b| {
        b.iter(|| {
            let mut s = String::from("Short"); // SSO
            for _ in 0..20 {
                s.push('x'); // Will transition to heap
            }
            black_box(s);
        });
    });
    
    group.bench_function("std_small_to_large", |b| {
        b.iter(|| {
            let mut s = std::string::String::from("Short");
            for _ in 0..20 {
                s.push('x');
            }
            black_box(s);
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_string_creation,
    bench_string_push,
    bench_string_concatenation,
    bench_string_builder,
    bench_string_search,
    bench_string_manipulation,
    bench_string_utf8,
    bench_sso_performance
);
criterion_main!(benches);