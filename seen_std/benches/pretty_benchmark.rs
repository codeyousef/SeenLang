//! Performance benchmarks for pretty printing
//!
//! Verifies pretty printing performance targets for compiler diagnostics

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use seen_std::pretty_simple::{PrettyPrinter, PrettyConfig};

fn bench_pretty_printing_simple(c: &mut Criterion) {
    let mut group = c.benchmark_group("pretty_printing_simple");
    
    let printer = PrettyPrinter::new();
    
    // Simple values
    let simple_values = vec![
        ("empty_object", "{}"),
        ("small_object", r#"{"name": "test", "value": 42}"#),
        ("empty_array", "[]"),
        ("small_array", r#"[1, 2, 3, 4, 5]"#),
        ("string", r#""Hello, World!""#),
        ("number", "123.456"),
        ("boolean", "true"),
        ("null", "null"),
    ];
    
    for (name, value) in simple_values {
        group.bench_function(name, |b| {
            b.iter(|| {
                let result = printer.format(black_box(value));
                black_box(result);
            });
        });
    }
    
    group.finish();
}

fn bench_pretty_printing_complex(c: &mut Criterion) {
    let mut group = c.benchmark_group("pretty_printing_complex");
    
    let printer = PrettyPrinter::new();
    
    // Complex nested structure (compiler diagnostic)
    let diagnostic = r#"{
        "level": "error",
        "message": "Type mismatch in function call",
        "code": "E0308",
        "location": {
            "file": "src/main.seen",
            "line": 42,
            "column": 15,
            "span": {
                "start": {"line": 42, "column": 15},
                "end": {"line": 42, "column": 25}
            }
        },
        "context": {
            "function": "calculate_sum",
            "expected_type": "Int",
            "actual_type": "String",
            "suggestion": "Convert string to integer using parse()"
        },
        "related": [
            {
                "level": "note",
                "message": "Function defined here",
                "location": {"file": "src/main.seen", "line": 10, "column": 3}
            }
        ],
        "source_lines": [
            "fn calculate_sum(a: Int, b: Int) -> Int {",
            "    return a + b;",
            "}",
            "",
            "fn main() {",
            "    let result = calculate_sum(\"10\", 20);",
            "                              ^^^^"
        ]
    }"#;
    
    group.bench_function("compiler_diagnostic", |b| {
        b.iter(|| {
            let result = printer.format(black_box(diagnostic));
            black_box(result);
        });
    });
    
    // Large array (symbol table dump)
    let symbol_table = format!("[{}]",
        (0..500).map(|i| format!(r#"{{
            "name": "symbol_{}",
            "type": "function",
            "visibility": "public",
            "location": {{"file": "lib.seen", "line": {}}},
            "signature": "fn(Int, String) -> Bool",
            "attributes": ["inline", "pure"]
        }}"#, i, i * 10))
        .collect::<Vec<_>>().join(",")
    );
    
    group.bench_function("large_symbol_table", |b| {
        b.iter(|| {
            let result = printer.format(black_box(&symbol_table));
            black_box(result);
        });
    });
    
    // Deep nesting (AST representation)
    let deep_ast = r#"{
        "type": "FunctionDecl",
        "name": "main",
        "body": {
            "type": "Block",
            "statements": [
                {
                    "type": "VariableDecl",
                    "name": "result",
                    "init": {
                        "type": "CallExpr",
                        "callee": {
                            "type": "Identifier",
                            "name": "calculate"
                        },
                        "args": [
                            {
                                "type": "BinaryExpr",
                                "operator": "+",
                                "left": {
                                    "type": "Literal",
                                    "value": 10
                                },
                                "right": {
                                    "type": "BinaryExpr",
                                    "operator": "*",
                                    "left": {
                                        "type": "Identifier",
                                        "name": "x"
                                    },
                                    "right": {
                                        "type": "Literal",
                                        "value": 2
                                    }
                                }
                            }
                        ]
                    }
                }
            ]
        }
    }"#;
    
    group.bench_function("deep_ast_nesting", |b| {
        b.iter(|| {
            let result = printer.format(black_box(deep_ast));
            black_box(result);
        });
    });
    
    group.finish();
}

fn bench_pretty_config_variations(c: &mut Criterion) {
    let mut group = c.benchmark_group("pretty_config_variations");
    
    let test_object = r#"{
        "compiler": {
            "version": "0.1.0",
            "features": ["debug", "optimize", "parallel"],
            "targets": ["native", "wasm", "mobile"],
            "settings": {
                "max_errors": 10,
                "warning_level": 3,
                "parallel_jobs": 8,
                "optimization_level": 2
            }
        },
        "files": [
            {"path": "main.seen", "size": 1234},
            {"path": "lib.seen", "size": 5678},
            {"path": "test.seen", "size": 2468}
        ]
    }"#;
    
    let configs = vec![
        ("indent_2", PrettyConfig { indent_size: 2, max_width: 80, compact: false }),
        ("indent_4", PrettyConfig { indent_size: 4, max_width: 80, compact: false }),
        ("width_60", PrettyConfig { indent_size: 2, max_width: 60, compact: false }),
        ("width_120", PrettyConfig { indent_size: 2, max_width: 120, compact: false }),
        ("compact", PrettyConfig { indent_size: 2, max_width: 80, compact: true }),
    ];
    
    for (name, config) in configs {
        group.bench_function(name, |b| {
            let printer = PrettyPrinter::with_config(config);
            b.iter(|| {
                let result = printer.format(black_box(test_object));
                black_box(result);
            });
        });
    }
    
    group.finish();
}

fn bench_pretty_error_messages(c: &mut Criterion) {
    let mut group = c.benchmark_group("pretty_error_messages");
    
    let printer = PrettyPrinter::new();
    
    // Simulate formatting multiple error messages (batch compilation)
    let errors = vec![
        r#"{"level": "error", "message": "Undefined variable 'x'", "line": 10}"#,
        r#"{"level": "error", "message": "Type mismatch", "line": 15}"#,
        r#"{"level": "warning", "message": "Unused import", "line": 5}"#,
        r#"{"level": "error", "message": "Missing semicolon", "line": 22}"#,
        r#"{"level": "warning", "message": "Dead code", "line": 30}"#,
    ];
    
    group.bench_function("batch_error_formatting", |b| {
        b.iter(|| {
            let mut results = Vec::new();
            for error in &errors {
                results.push(printer.format(black_box(error)));
            }
            black_box(results);
        });
    });
    
    // Large error batch (100 errors)
    let large_error_batch: Vec<String> = (0..100).map(|i| {
        format!(r#"{{"level": "error", "message": "Error #{}", "line": {}, "column": {}}}"#, i, i * 10, i * 5)
    }).collect();
    
    group.bench_function("large_error_batch", |b| {
        b.iter(|| {
            let mut results = Vec::new();
            for error in &large_error_batch {
                results.push(printer.format(black_box(error)));
            }
            black_box(results);
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_pretty_printing_simple,
    bench_pretty_printing_complex,
    bench_pretty_config_variations,
    bench_pretty_error_messages
);
criterion_main!(benches);