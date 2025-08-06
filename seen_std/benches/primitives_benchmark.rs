//! Performance benchmarks for primitive types
//!
//! Verifies zero-cost abstractions and optimal performance

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use seen_std::core::primitives::*;

fn bench_integer_arithmetic(c: &mut Criterion) {
    let mut group = c.benchmark_group("integer_arithmetic");
    
    group.bench_function("seen_i32", |b| {
        let a = I32::new(100);
        let b = I32::new(200);
        b.iter(|| {
            let mut sum = I32::new(0);
            for _ in 0..1000 {
                sum = sum + a * b - b / a;
            }
            black_box(sum);
        });
    });
    
    group.bench_function("native_i32", |b| {
        let a = 100i32;
        let b = 200i32;
        b.iter(|| {
            let mut sum = 0i32;
            for _ in 0..1000 {
                sum = sum + a * b - b / a;
            }
            black_box(sum);
        });
    });
    
    group.bench_function("seen_i64", |b| {
        let a = I64::new(100);
        let b = I64::new(200);
        b.iter(|| {
            let mut sum = I64::new(0);
            for _ in 0..1000 {
                sum = sum + a * b - b / a;
            }
            black_box(sum);
        });
    });
    
    group.bench_function("native_i64", |b| {
        let a = 100i64;
        let b = 200i64;
        b.iter(|| {
            let mut sum = 0i64;
            for _ in 0..1000 {
                sum = sum + a * b - b / a;
            }
            black_box(sum);
        });
    });
    
    group.finish();
}

fn bench_float_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("float_operations");
    
    group.bench_function("seen_f32", |b| {
        let a = F32::new(3.14);
        let b = F32::new(2.71);
        b.iter(|| {
            let mut result = F32::new(1.0);
            for _ in 0..100 {
                result = (result * a + b).sqrt();
            }
            black_box(result);
        });
    });
    
    group.bench_function("native_f32", |b| {
        let a = 3.14f32;
        let b = 2.71f32;
        b.iter(|| {
            let mut result = 1.0f32;
            for _ in 0..100 {
                result = (result * a + b).sqrt();
            }
            black_box(result);
        });
    });
    
    group.bench_function("seen_f64", |b| {
        let a = F64::new(3.14);
        let b = F64::new(2.71);
        b.iter(|| {
            let mut result = F64::new(1.0);
            for _ in 0..100 {
                result = (result * a + b).sqrt();
            }
            black_box(result);
        });
    });
    
    group.bench_function("native_f64", |b| {
        let a = 3.14f64;
        let b = 2.71f64;
        b.iter(|| {
            let mut result = 1.0f64;
            for _ in 0..100 {
                result = (result * a + b).sqrt();
            }
            black_box(result);
        });
    });
    
    group.finish();
}

fn bench_bitwise_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("bitwise_operations");
    
    group.bench_function("seen_u32", |b| {
        let a = U32::new(0xDEADBEEF);
        let b = U32::new(0xCAFEBABE);
        b.iter(|| {
            let mut result = U32::new(0);
            for _ in 0..1000 {
                result = (result ^ a) & b | (a << 3) ^ (b >> 2);
            }
            black_box(result);
        });
    });
    
    group.bench_function("native_u32", |b| {
        let a = 0xDEADBEEFu32;
        let b = 0xCAFEBABEu32;
        b.iter(|| {
            let mut result = 0u32;
            for _ in 0..1000 {
                result = (result ^ a) & b | (a << 3) ^ (b >> 2);
            }
            black_box(result);
        });
    });
    
    group.finish();
}

fn bench_bool_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("bool_operations");
    
    group.bench_function("seen_bool", |b| {
        let t = Bool::TRUE;
        let f = Bool::FALSE;
        b.iter(|| {
            let mut result = Bool::FALSE;
            for i in 0..1000 {
                result = if i % 2 == 0 { result & t } else { result | f } ^ t;
            }
            black_box(result);
        });
    });
    
    group.bench_function("native_bool", |b| {
        let t = true;
        let f = false;
        b.iter(|| {
            let mut result = false;
            for i in 0..1000 {
                result = if i % 2 == 0 { result & t } else { result | f } ^ t;
            }
            black_box(result);
        });
    });
    
    group.finish();
}

fn bench_char_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("char_operations");
    
    let test_chars = vec!['a', 'Z', '5', '😀', '世', '\n'];
    
    group.bench_function("seen_char", |b| {
        let chars: Vec<Char> = test_chars.iter().map(|&c| Char::new(c)).collect();
        b.iter(|| {
            let mut count = 0;
            for _ in 0..100 {
                for ch in &chars {
                    if ch.is_alphabetic() {
                        count += 1;
                    }
                    if ch.is_numeric() {
                        count += 2;
                    }
                    black_box(ch.to_uppercase());
                    black_box(ch.to_lowercase());
                }
            }
            black_box(count);
        });
    });
    
    group.bench_function("native_char", |b| {
        let chars = &test_chars;
        b.iter(|| {
            let mut count = 0;
            for _ in 0..100 {
                for &ch in chars {
                    if ch.is_alphabetic() {
                        count += 1;
                    }
                    if ch.is_numeric() {
                        count += 2;
                    }
                    black_box(ch.to_uppercase());
                    black_box(ch.to_lowercase());
                }
            }
            black_box(count);
        });
    });
    
    group.finish();
}

fn bench_conversion_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("conversion_overhead");
    
    group.bench_function("seen_conversions", |b| {
        b.iter(|| {
            let mut sum = 0i64;
            for i in 0..1000 {
                let a = I32::new(i as i32);
                let b = I64::from(a);
                sum += b.value();
            }
            black_box(sum);
        });
    });
    
    group.bench_function("native_conversions", |b| {
        b.iter(|| {
            let mut sum = 0i64;
            for i in 0..1000 {
                let a = i as i32;
                let b = a as i64;
                sum += b;
            }
            black_box(sum);
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_integer_arithmetic,
    bench_float_operations,
    bench_bitwise_operations,
    bench_bool_operations,
    bench_char_operations,
    bench_conversion_overhead
);
criterion_main!(benches);