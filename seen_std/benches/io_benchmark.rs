//! I/O performance benchmarks
//!
//! Tests that our I/O system achieves full bandwidth and beats C stdio performance

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use seen_std::io::*;
use seen_std::string::String;
use std::fs;
use tempfile::TempDir;

fn create_large_test_file(size_mb: usize) -> (TempDir, std::path::PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("large_test.dat");
    
    // Create test data - repeated pattern for better compression testing
    let chunk = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ\n";
    let chunk_size = chunk.len();
    let total_chunks = (size_mb * 1024 * 1024) / chunk_size;
    
    let content = chunk.repeat(total_chunks);
    fs::write(&file_path, content).unwrap();
    
    (temp_dir, file_path)
}

fn bench_file_read_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_read_throughput");
    group.measurement_time(std::time::Duration::from_secs(10));
    group.sample_size(20);
    
    // Test with different file sizes
    for &size_mb in &[1, 10, 50] {
        let (_temp_dir, file_path) = create_large_test_file(size_mb);
        
        group.bench_with_input(
            BenchmarkId::new("seen_read_to_string", size_mb),
            &size_mb,
            |b, &_size| {
                b.iter(|| {
                    let mut file = File::open(&file_path).expect("Failed to open file");
                    let content = file.read_to_string().expect("Failed to read");
                    black_box(content.len());
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("std_read_to_string", size_mb),
            &size_mb,
            |b, &_size| {
                b.iter(|| {
                    let content = fs::read_to_string(&file_path).expect("Failed to read");
                    black_box(content.len());
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("seen_read_to_bytes", size_mb),
            &size_mb,
            |b, &_size| {
                b.iter(|| {
                    let mut file = File::open(&file_path).expect("Failed to open file");
                    let bytes = file.read_to_bytes().expect("Failed to read");
                    black_box(bytes.len());
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("std_read_bytes", size_mb),
            &size_mb,
            |b, &_size| {
                b.iter(|| {
                    let bytes = fs::read(&file_path).expect("Failed to read");
                    black_box(bytes.len());
                });
            },
        );
    }
    
    group.finish();
}

fn bench_file_write_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_write_throughput");
    group.measurement_time(std::time::Duration::from_secs(10));
    group.sample_size(20);
    
    // Create test data
    let chunk = "The quick brown fox jumps over the lazy dog. ";
    let test_data_1mb = String::from(&chunk.repeat(1024 * 1024 / chunk.len()));
    let test_bytes_1mb: Vec<u8> = chunk.repeat(1024 * 1024 / chunk.len()).into_bytes();
    
    for &size_mb in &[1, 10] {
        let test_data = String::from(&chunk.repeat((size_mb * 1024 * 1024) / chunk.len()));
        let test_bytes: Vec<u8> = chunk.repeat((size_mb * 1024 * 1024) / chunk.len()).into_bytes();
        
        group.bench_with_input(
            BenchmarkId::new("seen_write_string", size_mb),
            &size_mb,
            |b, &_size| {
                b.iter(|| {
                    let temp_dir = TempDir::new().unwrap();
                    let file_path = temp_dir.path().join("write_test.txt");
                    let mut file = File::create(&file_path).expect("Failed to create");
                    file.write_string(&test_data).expect("Failed to write");
                    file.flush().expect("Failed to flush");
                    black_box(test_data.len());
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("std_write_string", size_mb),
            &size_mb,
            |b, &_size| {
                b.iter(|| {
                    let temp_dir = TempDir::new().unwrap();
                    let file_path = temp_dir.path().join("write_test.txt");
                    fs::write(&file_path, test_data.as_str()).expect("Failed to write");
                    black_box(test_data.len());
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("seen_write_bytes", size_mb),
            &size_mb,
            |b, &_size| {
                b.iter(|| {
                    let temp_dir = TempDir::new().unwrap();
                    let file_path = temp_dir.path().join("write_test.bin");
                    let mut file = File::create(&file_path).expect("Failed to create");
                    file.write_bytes(&test_bytes).expect("Failed to write");
                    file.flush().expect("Failed to flush");
                    black_box(test_bytes.len());
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("std_write_bytes", size_mb),
            &size_mb,
            |b, &_size| {
                b.iter(|| {
                    let temp_dir = TempDir::new().unwrap();
                    let file_path = temp_dir.path().join("write_test.bin");
                    fs::write(&file_path, &test_bytes).expect("Failed to write");
                    black_box(test_bytes.len());
                });
            },
        );
    }
    
    group.finish();
}

fn bench_buffered_io(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffered_io");
    
    // Test small, frequent operations where buffering should help
    group.bench_function("seen_many_small_writes", |b| {
        b.iter(|| {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("buffered_test.txt");
            let mut file = File::create(&file_path).expect("Failed to create");
            
            // Write 10000 small strings
            for i in 0..10000 {
                let line = String::from(&format!("Line {}\n", i));
                file.write_string(&line).expect("Failed to write");
            }
            file.flush().expect("Failed to flush");
            black_box(10000);
        });
    });
    
    group.bench_function("std_many_small_writes", |b| {
        b.iter(|| {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("buffered_test.txt");
            let mut content = std::string::String::new();
            
            // Build string then write once
            for i in 0..10000 {
                content.push_str(&format!("Line {}\n", i));
            }
            fs::write(&file_path, content).expect("Failed to write");
            black_box(10000);
        });
    });
    
    group.finish();
}

fn bench_file_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_operations");
    
    let temp_dir = TempDir::new().unwrap();
    let test_content = "Test file content for operations";
    
    group.bench_function("seen_copy_file", |b| {
        let source = temp_dir.path().join("source.txt");
        fs::write(&source, test_content).unwrap();
        
        b.iter(|| {
            let dest = temp_dir.path().join(format!("dest_{}.txt", rand::random::<u32>()));
            let bytes_copied = copy_file(&source, &dest).expect("Failed to copy");
            black_box(bytes_copied);
        });
    });
    
    group.bench_function("std_copy_file", |b| {
        let source = temp_dir.path().join("std_source.txt");
        fs::write(&source, test_content).unwrap();
        
        b.iter(|| {
            let dest = temp_dir.path().join(format!("std_dest_{}.txt", rand::random::<u32>()));
            let bytes_copied = fs::copy(&source, &dest).expect("Failed to copy");
            black_box(bytes_copied);
        });
    });
    
    group.bench_function("seen_file_exists", |b| {
        let existing_file = temp_dir.path().join("existing.txt");
        let nonexistent_file = temp_dir.path().join("nonexistent.txt");
        fs::write(&existing_file, test_content).unwrap();
        
        b.iter(|| {
            let exists1 = file_exists(&existing_file);
            let exists2 = file_exists(&nonexistent_file);
            black_box((exists1, exists2));
        });
    });
    
    group.bench_function("std_file_exists", |b| {
        let existing_file = temp_dir.path().join("std_existing.txt");
        let nonexistent_file = temp_dir.path().join("std_nonexistent.txt");
        fs::write(&existing_file, test_content).unwrap();
        
        b.iter(|| {
            let exists1 = existing_file.exists();
            let exists2 = nonexistent_file.exists();
            black_box((exists1, exists2));
        });
    });
    
    group.finish();
}

fn bench_seek_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("seek_operations");
    
    // Create a larger file for seeking tests
    let (_temp_dir, file_path) = create_large_test_file(10); // 10MB file
    
    group.bench_function("seen_random_seeks", |b| {
        b.iter(|| {
            let mut file = File::open(&file_path).expect("Failed to open");
            let mut sum = 0u64;
            
            // Perform 1000 random seeks and position queries
            for i in 0..1000 {
                let pos = (i * 97) % (10 * 1024 * 1024); // Random position
                file.seek(std::io::SeekFrom::Start(pos)).expect("Failed to seek");
                let current = file.position().expect("Failed to get position");
                sum += current;
            }
            black_box(sum);
        });
    });
    
    group.bench_function("std_random_seeks", |b| {
        use std::io::{Seek, SeekFrom};
        
        b.iter(|| {
            let mut file = fs::File::open(&file_path).expect("Failed to open");
            let mut sum = 0u64;
            
            for i in 0..1000 {
                let pos = (i * 97) % (10 * 1024 * 1024);
                file.seek(SeekFrom::Start(pos)).expect("Failed to seek");
                let current = file.stream_position().expect("Failed to get position");
                sum += current;
            }
            black_box(sum);
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_file_read_throughput,
    bench_file_write_throughput,
    bench_buffered_io,
    bench_file_operations,
    bench_seek_operations
);
criterion_main!(benches);