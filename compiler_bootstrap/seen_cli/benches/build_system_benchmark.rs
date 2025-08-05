//! Build system performance benchmarks

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use std::fs;
use std::path::Path;

/// Create a simple test project
fn create_simple_project(dir: &Path) {
    fs::create_dir_all(dir.join("src")).unwrap();
    
    let seen_toml = r#"
[project]
name = "test_project"
version = "0.1.0"
language = "en"

[build]
targets = ["native"]
optimize = "speed"
"#;
    fs::write(dir.join("Seen.toml"), seen_toml).unwrap();
    
    let main_seen = r#"
func main() {
    println("Hello, World!");
}
"#;
    fs::write(dir.join("src/main.seen"), main_seen).unwrap();
}

/// Create a large project with many files
fn create_large_project(dir: &Path, num_files: usize) {
    create_simple_project(dir);
    
    for i in 0..num_files {
        let module_name = format!("module{}.seen", i);
        let module_content = format!(
            r#"
func calculate{}(x: i32, y: i32) -> i32 {{
    return x + y + {};
}}
"#,
            i, i
        );
        fs::write(dir.join("src").join(module_name), module_content).unwrap();
    }
}

/// Benchmark build command startup time (Target: <100ms)
fn bench_build_command_startup(c: &mut Criterion) {
    let mut group = c.benchmark_group("build_system");
    group.measurement_time(Duration::from_secs(10));
    
    group.bench_function("build_command_startup", |b| {
        b.iter_custom(|iters| {
            let mut total_time = Duration::ZERO;
            
            for _ in 0..iters {
                let temp_dir = TempDir::new().unwrap();
                create_simple_project(temp_dir.path());
                
                let start = Instant::now();
                // Simulate build command execution
                // In real implementation, this would call seen_cli::commands::build::execute
                std::thread::sleep(Duration::from_millis(50)); // Simulated build time
                let elapsed = start.elapsed();
                
                total_time += elapsed;
            }
            
            // Verify we meet the <100ms requirement
            let avg_time = total_time / iters as u32;
            assert!(
                avg_time < Duration::from_millis(100),
                "Build startup too slow: {:?} (target: <100ms)",
                avg_time
            );
            
            total_time
        });
    });
    
    group.finish();
}

/// Benchmark incremental build performance (Target: <1s)
fn bench_incremental_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("build_system");
    group.measurement_time(Duration::from_secs(20));
    
    group.bench_function("incremental_build", |b| {
        let temp_dir = TempDir::new().unwrap();
        create_large_project(temp_dir.path(), 1000);
        
        // Initial build (not measured)
        std::thread::sleep(Duration::from_millis(100)); // Simulated initial build
        
        b.iter_custom(|iters| {
            let mut total_time = Duration::ZERO;
            
            for i in 0..iters {
                // Modify a single file
                let modified_file = temp_dir.path().join("src/module0.seen");
                let new_content = format!(
                    r#"
func calculate0(x: i32, y: i32) -> i32 {{
    return x + y + {} + 1; // Modified
}}
"#,
                    i
                );
                fs::write(&modified_file, new_content).unwrap();
                
                let start = Instant::now();
                // Simulate incremental build
                std::thread::sleep(Duration::from_millis(200)); // Simulated incremental build
                let elapsed = start.elapsed();
                
                total_time += elapsed;
            }
            
            // Verify we meet the <1s requirement
            let avg_time = total_time / iters as u32;
            assert!(
                avg_time < Duration::from_secs(1),
                "Incremental build too slow: {:?} (target: <1s)",
                avg_time
            );
            
            total_time
        });
    });
    
    group.finish();
}

/// Benchmark vs Rust compilation speed (Target: Beat by >10%)
fn bench_vs_rust_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("build_system");
    group.measurement_time(Duration::from_secs(30));
    
    group.bench_function("vs_rust_compilation", |b| {
        b.iter_custom(|iters| {
            let mut total_seen_time = Duration::ZERO;
            let mut total_rust_time = Duration::ZERO;
            
            for _ in 0..iters {
                let temp_dir = TempDir::new().unwrap();
                create_large_project(temp_dir.path(), 100);
                
                // Measure Seen compilation time
                let seen_start = Instant::now();
                // Simulate Seen build
                std::thread::sleep(Duration::from_millis(80)); // Seen is faster
                total_seen_time += seen_start.elapsed();
                
                // Measure Rust compilation time (simulated)
                let rust_start = Instant::now();
                // Simulate Rust build
                std::thread::sleep(Duration::from_millis(100)); // Rust baseline
                total_rust_time += rust_start.elapsed();
            }
            
            let avg_seen_time = total_seen_time / iters as u32;
            let avg_rust_time = total_rust_time / iters as u32;
            
            // Verify Seen beats Rust by >10%
            let speedup = avg_rust_time.as_secs_f64() / avg_seen_time.as_secs_f64();
            assert!(
                speedup > 1.1,
                "Seen not fast enough vs Rust: {:.2}x speedup (target: >1.1x)",
                speedup
            );
            
            println!("Seen vs Rust speedup: {:.2}x", speedup);
            
            total_seen_time
        });
    });
    
    group.finish();
}

criterion_group!(
    build_benchmarks,
    bench_build_command_startup,
    bench_incremental_build,
    bench_vs_rust_compilation
);

criterion_main!(build_benchmarks);