//! Performance benchmarks for TOML parsing
//!
//! Verifies TOML performance targets for configuration files

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use seen_std::toml::{TomlValue, parse_toml};

fn bench_toml_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("toml_parsing");
    
    // Small TOML config
    let small_toml = r#"
# Seen compiler configuration
name = "seen-compiler"
version = "0.1.0"
edition = "2024"

[compiler]
optimize = true
debug = false
warnings = "all"
max_errors = 10

[targets]
native = true
wasm = false
mobile = false

[dependencies]
std = "0.1.0"
collections = "0.1.0"
"#;
    
    // Large TOML config (realistic project configuration)
    let large_toml = format!(r#"
# Large project configuration
name = "large-project"
version = "1.0.0"
authors = ["dev1@example.com", "dev2@example.com"]
description = "A large Seen project"
license = "MIT"

[compiler]
optimize = true
debug = false
warnings = "all"
max_errors = 50
parallel = 8
target_dir = "target"

[targets]
native = true
wasm = true
mobile = true
embedded = false

{}

[build]
jobs = 8
features = ["full", "parallel", "optimize"]
rustflags = ["-C", "target-cpu=native"]

[profile.release]
opt_level = 3
debug = false
lto = true
codegen_units = 1
panic = "abort"

[profile.debug]
opt_level = 0
debug = true
lto = false
panic = "unwind"
"#, 
        (0..20).map(|i| format!("[dependencies.dep_{}]\nversion = \"{}.0.0\"\nfeatures = [\"default\", \"extra\"]\noptional = false", i, i))
            .collect::<Vec<_>>().join("\n\n")
    );
    
    group.bench_function("small_config", |b| {
        b.iter(|| {
            let result = parse_toml(black_box(small_toml));
            black_box(result);
        });
    });
    
    group.bench_function("large_config", |b| {
        b.iter(|| {
            let result = parse_toml(black_box(&large_toml));
            black_box(result);
        });
    });
    
    // Very complex nested TOML
    let complex_toml = r#"
# Complex nested configuration
[database]
host = "localhost"
port = 5432
name = "seen_db"
ssl = true

[database.pool]
max_connections = 100
min_connections = 5
timeout = 30

[database.migrations]
auto = true
directory = "migrations"
table = "schema_migrations"

[server]
host = "0.0.0.0"
port = 8080
workers = 4

[server.tls]
cert = "cert.pem"
key = "key.pem"
protocols = ["http/1.1", "h2"]

[server.cors]
origins = ["*"]
methods = ["GET", "POST", "PUT", "DELETE"]
headers = ["Authorization", "Content-Type"]

[logging]
level = "info"
format = "json"

[logging.file]
path = "logs/app.log"
max_size = "100MB"
rotate = 7

[metrics]
enabled = true
namespace = "seen"
tags = { environment = "production", service = "compiler" }

[cache]
type = "redis"
url = "redis://localhost:6379"
ttl = 3600
max_entries = 10000
"#;
    
    group.bench_function("complex_nested", |b| {
        b.iter(|| {
            let result = parse_toml(black_box(complex_toml));
            black_box(result);
        });
    });
    
    group.finish();
}

fn bench_toml_value_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("toml_value_access");
    
    let toml_content = r#"
[compiler]
optimize = true
debug = false
warnings = "all"
max_errors = 10
parallel = 8

[targets]
native = true
wasm = false
mobile = false

[dependencies]
std = "0.1.0"
collections = "0.1.0"
io = "0.1.0"
"#;
    
    let parsed = parse_toml(toml_content).unwrap();
    
    group.bench_function("nested_access", |b| {
        b.iter(|| {
            let optimize = parsed.get_bool(&["compiler", "optimize"]);
            let warnings = parsed.get_string(&["compiler", "warnings"]);
            let native = parsed.get_bool(&["targets", "native"]);
            let std_version = parsed.get_string(&["dependencies", "std"]);
            black_box((optimize, warnings, native, std_version));
        });
    });
    
    group.bench_function("table_iteration", |b| {
        b.iter(|| {
            if let Some(deps) = parsed.get_table(&["dependencies"]) {
                let mut count = 0;
                for (key, _value) in deps {
                    count += key.len();
                }
                black_box(count);
            }
        });
    });
    
    group.finish();
}

fn bench_toml_error_recovery(c: &mut Criterion) {
    let mut group = c.benchmark_group("toml_error_recovery");
    
    // TOML with various syntax errors
    let invalid_tomls = vec![
        "key = \"unclosed string",
        "key = [1, 2, 3",
        "[[invalid.table.name",
        "key = value without quotes",
        "duplicate = 1\nduplicate = 2",
        "[table\nkey = value",
    ];
    
    for (i, invalid_toml) in invalid_tomls.iter().enumerate() {
        group.bench_with_input(BenchmarkId::new("error_recovery", i), invalid_toml, |b, &toml| {
            b.iter(|| {
                let result = parse_toml(black_box(toml));
                black_box(result);
            });
        });
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_toml_parsing,
    bench_toml_value_access,
    bench_toml_error_recovery
);
criterion_main!(benches);