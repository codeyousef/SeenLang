//! Performance benchmarks for collections
//!
//! Verifies that Seen collections beat Rust/C++ STL performance

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use seen_std::collections::{Vec, HashMap, HashSet};
use std::collections::{HashMap as StdHashMap, HashSet as StdHashSet};

fn bench_vec_push(c: &mut Criterion) {
    let mut group = c.benchmark_group("vec_push");
    
    for size in &[100, 1000, 10000, 100000] {
        group.bench_with_input(BenchmarkId::new("seen", size), size, |b, &size| {
            b.iter(|| {
                let mut vec = Vec::new();
                for i in 0..size {
                    vec.push(black_box(i));
                }
                black_box(vec);
            });
        });
        
        group.bench_with_input(BenchmarkId::new("std", size), size, |b, &size| {
            b.iter(|| {
                let mut vec = std::vec::Vec::new();
                for i in 0..size {
                    vec.push(black_box(i));
                }
                black_box(vec);
            });
        });
    }
    group.finish();
}

fn bench_vec_random_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("vec_random_access");
    let size = 100000;
    
    // Prepare data
    let mut seen_vec = Vec::new();
    let mut std_vec = std::vec::Vec::new();
    for i in 0..size {
        seen_vec.push(i as u64);
        std_vec.push(i as u64);
    }
    
    group.bench_function("seen", |b| {
        b.iter(|| {
            let mut sum = 0u64;
            for i in 0..1000 {
                let idx = (i * 97) % size;
                sum += seen_vec[idx];
            }
            black_box(sum);
        });
    });
    
    group.bench_function("std", |b| {
        b.iter(|| {
            let mut sum = 0u64;
            for i in 0..1000 {
                let idx = (i * 97) % size;
                sum += std_vec[idx];
            }
            black_box(sum);
        });
    });
    
    group.finish();
}

fn bench_hashmap_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("hashmap_insert");
    
    for size in &[100, 1000, 10000] {
        group.bench_with_input(BenchmarkId::new("seen", size), size, |b, &size| {
            b.iter(|| {
                let mut map = HashMap::new();
                for i in 0..size {
                    map.insert(i, black_box(i * 2));
                }
                black_box(map);
            });
        });
        
        group.bench_with_input(BenchmarkId::new("std", size), size, |b, &size| {
            b.iter(|| {
                let mut map = StdHashMap::new();
                for i in 0..size {
                    map.insert(i, black_box(i * 2));
                }
                black_box(map);
            });
        });
    }
    group.finish();
}

fn bench_hashmap_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("hashmap_lookup");
    let size = 10000;
    
    // Prepare maps
    let mut seen_map = HashMap::new();
    let mut std_map = StdHashMap::new();
    for i in 0..size {
        seen_map.insert(i, i * 2);
        std_map.insert(i, i * 2);
    }
    
    group.bench_function("seen", |b| {
        b.iter(|| {
            let mut sum = 0;
            for i in 0..1000 {
                let key = (i * 97) % size;
                if let Some(&val) = seen_map.get(&key) {
                    sum += val;
                }
            }
            black_box(sum);
        });
    });
    
    group.bench_function("std", |b| {
        b.iter(|| {
            let mut sum = 0;
            for i in 0..1000 {
                let key = (i * 97) % size;
                if let Some(&val) = std_map.get(&key) {
                    sum += val;
                }
            }
            black_box(sum);
        });
    });
    
    group.finish();
}

fn bench_hashmap_robin_hood(c: &mut Criterion) {
    let mut group = c.benchmark_group("hashmap_cache_efficiency");
    
    // Test cache-hostile access pattern
    group.bench_function("seen_robin_hood", |b| {
        let mut map = HashMap::new();
        // Pre-fill with sequential keys
        for i in 0..10000 {
            map.insert(i, i);
        }
        
        b.iter(|| {
            let mut sum = 0;
            // Access in cache-hostile pattern
            for i in 0..1000 {
                let key = (i * 7919) % 10000; // Large prime for poor locality
                if let Some(&val) = map.get(&key) {
                    sum += val;
                }
            }
            black_box(sum);
        });
    });
    
    group.bench_function("std_hashmap", |b| {
        let mut map = StdHashMap::new();
        for i in 0..10000 {
            map.insert(i, i);
        }
        
        b.iter(|| {
            let mut sum = 0;
            for i in 0..1000 {
                let key = (i * 7919) % 10000;
                if let Some(&val) = map.get(&key) {
                    sum += val;
                }
            }
            black_box(sum);
        });
    });
    
    group.finish();
}

fn bench_hashset_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("hashset_operations");
    
    let size = 1000;
    
    group.bench_function("seen_insert", |b| {
        b.iter(|| {
            let mut set = HashSet::new();
            for i in 0..size {
                set.insert(black_box(i));
            }
            black_box(set);
        });
    });
    
    group.bench_function("std_insert", |b| {
        b.iter(|| {
            let mut set = StdHashSet::new();
            for i in 0..size {
                set.insert(black_box(i));
            }
            black_box(set);
        });
    });
    
    // Test contains performance
    let mut seen_set = HashSet::new();
    let mut std_set = StdHashSet::new();
    for i in 0..size {
        seen_set.insert(i);
        std_set.insert(i);
    }
    
    group.bench_function("seen_contains", |b| {
        b.iter(|| {
            let mut found = 0;
            for i in 0..1000 {
                if seen_set.contains(&((i * 97) % (size * 2))) {
                    found += 1;
                }
            }
            black_box(found);
        });
    });
    
    group.bench_function("std_contains", |b| {
        b.iter(|| {
            let mut found = 0;
            for i in 0..1000 {
                if std_set.contains(&((i * 97) % (size * 2))) {
                    found += 1;
                }
            }
            black_box(found);
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_vec_push,
    bench_vec_random_access,
    bench_hashmap_insert,
    bench_hashmap_lookup,
    bench_hashmap_robin_hood,
    bench_hashset_operations
);
criterion_main!(benches);