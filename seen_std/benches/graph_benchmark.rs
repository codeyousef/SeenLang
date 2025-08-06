//! Performance benchmarks for graph algorithms
//!
//! Verifies graph algorithm performance targets are met

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use seen_std::graph::{Graph, ModuleGraph};

fn bench_topological_sort(c: &mut Criterion) {
    let mut group = c.benchmark_group("topological_sort");
    
    for size in &[10, 100, 1000, 5000] {
        group.bench_with_input(BenchmarkId::new("linear_chain", size), size, |b, &size| {
            let mut graph = Graph::new();
            // Create linear dependency chain
            for i in 0..size {
                if i > 0 {
                    graph.add_edge(i - 1, i);
                }
            }
            
            b.iter(|| {
                let result = graph.topological_sort();
                black_box(result);
            });
        });

        group.bench_with_input(BenchmarkId::new("complex_deps", size), size, |b, &size| {
            let mut graph = Graph::new();
            // Create complex dependency structure
            for i in 0..size {
                if i > 0 {
                    graph.add_edge(i - 1, i);
                }
                if i > 10 {
                    graph.add_edge(i - 10, i);
                }
                if i > 50 {
                    graph.add_edge(i - 50, i);
                }
            }
            
            b.iter(|| {
                let result = graph.topological_sort();
                black_box(result);
            });
        });
    }
    
    group.finish();
}

fn bench_strongly_connected_components(c: &mut Criterion) {
    let mut group = c.benchmark_group("strongly_connected_components");
    
    for size in &[10, 100, 500] {
        group.bench_with_input(BenchmarkId::new("tarjan_scc", size), size, |b, &size| {
            let mut graph = Graph::new();
            // Create graph with multiple SCCs
            for i in 0..size {
                let next = (i + 1) % 10;
                graph.add_edge(i, next); // Create cycles of size 10
                if i % 10 == 0 && i > 0 {
                    graph.add_edge(i - 10, i); // Connect SCCs
                }
            }
            
            b.iter(|| {
                let result = graph.strongly_connected_components();
                black_box(result);
            });
        });
    }
    
    group.finish();
}

fn bench_module_graph_compilation_order(c: &mut Criterion) {
    let mut group = c.benchmark_group("module_compilation_order");
    
    for size in &[10, 100, 500] {
        group.bench_with_input(BenchmarkId::new("realistic_compiler", size), size, |b, &size| {
            let mut graph = ModuleGraph::new();
            
            // Create realistic compiler module structure
            for i in 0..size {
                let module = format!("module_{}", i);
                if i > 0 {
                    let dep = format!("module_{}", i - 1);
                    graph.add_dependency(&module, &dep);
                }
                if i > 5 {
                    let dep = format!("module_{}", i - 5);
                    graph.add_dependency(&module, &dep);
                }
                // Add common dependencies
                if i % 10 == 0 && i > 0 {
                    graph.add_dependency(&module, "std");
                    graph.add_dependency(&module, "collections");
                }
            }
            
            b.iter(|| {
                let result = graph.compilation_order();
                black_box(result);
            });
        });
    }
    
    group.finish();
}

fn bench_dependency_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("dependency_analysis");
    
    let mut graph = ModuleGraph::new();
    // Create a large realistic dependency graph
    for i in 0..1000 {
        let module = format!("module_{}", i);
        if i > 0 {
            let dep = format!("module_{}", i - 1);
            graph.add_dependency(&module, &dep);
        }
        if i > 10 {
            let dep = format!("module_{}", i - 10);
            graph.add_dependency(&module, &dep);
        }
        if i > 100 {
            let dep = format!("module_{}", i - 100);
            graph.add_dependency(&module, &dep);
        }
    }
    
    group.bench_function("module_dependencies", |b| {
        b.iter(|| {
            let deps = graph.module_dependencies("module_500");
            black_box(deps);
        });
    });
    
    group.bench_function("module_dependents", |b| {
        b.iter(|| {
            let deps = graph.module_dependents("module_500");
            black_box(deps);
        });
    });
    
    group.bench_function("has_circular_dependencies", |b| {
        b.iter(|| {
            let has_cycles = graph.has_circular_dependencies();
            black_box(has_cycles);
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_topological_sort,
    bench_strongly_connected_components,
    bench_module_graph_compilation_order,
    bench_dependency_analysis
);
criterion_main!(benches);