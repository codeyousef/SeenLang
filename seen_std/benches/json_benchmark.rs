//! Performance benchmarks for JSON parsing and serialization
//!
//! Verifies JSON performance targets for compiler workloads

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use seen_std::json::{JsonValue, parse_json};
use seen_std::collections::{Vec, HashMap};
use seen_std::string::String;

fn bench_json_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_parsing");
    
    // Small JSON object
    let small_json = r#"{
        "name": "test",
        "version": "1.0.0",
        "enabled": true,
        "count": 42
    }"#;
    
    // Large JSON object (typical compiler API response)
    let large_json = format!(r#"{{
        "diagnostics": [{}],
        "symbols": [{}],
        "metadata": {{
            "compilation_time": 123.45,
            "memory_usage": 67890,
            "files_processed": 100,
            "warnings": {},
            "errors": {}
        }}
    }}"#,
        (0..50).map(|i| format!(r#"{{"message": "Warning {}", "line": {}, "column": {}}}"#, i, i*10, i*5)).collect::<Vec<_>>().join(","),
        (0..200).map(|i| format!(r#"{{"name": "symbol_{}", "type": "function", "line": {}}}"#, i, i*3)).collect::<Vec<_>>().join(","),
        25, 5
    );
    
    group.bench_function("small_object", |b| {
        b.iter(|| {
            let result = parse_json(black_box(small_json));
            black_box(result);
        });
    });
    
    group.bench_function("large_object", |b| {
        b.iter(|| {
            let result = parse_json(black_box(&large_json));
            black_box(result);
        });
    });
    
    // Array parsing
    let array_json = format!("[{}]", 
        (0..1000).map(|i| format!(r#"{{"id": {}, "value": "item_{}", "active": {}}}"#, i, i, i % 2 == 0))
        .collect::<Vec<_>>().join(",")
    );
    
    group.bench_function("large_array", |b| {
        b.iter(|| {
            let result = parse_json(black_box(&array_json));
            black_box(result);
        });
    });
    
    group.finish();
}

fn bench_json_value_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_value_access");
    
    // Parse test data once
    let json_data = r#"{
        "compiler": {
            "version": "0.1.0",
            "features": ["debug", "optimize", "parallel"],
            "settings": {
                "max_errors": 10,
                "warning_level": 3,
                "output_format": "json"
            }
        },
        "files": [
            {"path": "main.seen", "size": 1234, "modified": 1609459200},
            {"path": "lib.seen", "size": 5678, "modified": 1609459300},
            {"path": "test.seen", "size": 2468, "modified": 1609459400}
        ]
    }"#;
    
    let parsed = parse_json(json_data).unwrap();
    
    group.bench_function("nested_object_access", |b| {
        b.iter(|| {
            let compiler = parsed.get("compiler");
            if let Some(comp) = compiler {
                let version = comp.get("version");
                let settings = comp.get("settings");
                black_box((version, settings));
            }
        });
    });
    
    group.bench_function("array_iteration", |b| {
        b.iter(|| {
            if let Some(files) = parsed.get("files") {
                if let Some(arr) = files.as_array() {
                    let mut total_size = 0.0;
                    for file in arr {
                        if let Some(size_val) = file.get("size") {
                            if let Some(size) = size_val.as_number() {
                                total_size += size;
                            }
                        }
                    }
                    black_box(total_size);
                }
            }
        });
    });
    
    group.finish();
}

fn bench_json_error_handling(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_error_handling");
    
    let invalid_jsons = vec![
        r#"{"key": "unclosed string"#,
        r#"{"key": [1, 2, 3"#,
        r#"{"key": value}"#,
        r#"{"duplicate": 1, "duplicate": 2}"#,
        r#"{invalid_key: "value"}"#,
    ];
    
    group.bench_function("error_recovery", |b| {
        b.iter(|| {
            for invalid_json in &invalid_jsons {
                let result = parse_json(black_box(invalid_json));
                black_box(result);
            }
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_json_parsing,
    bench_json_value_access,
    bench_json_error_handling
);
criterion_main!(benches);