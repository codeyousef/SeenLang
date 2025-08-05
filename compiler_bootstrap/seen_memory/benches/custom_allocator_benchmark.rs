//! Custom allocator performance benchmarks

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::time::Duration;
use seen_memory::{RuntimeManager, MemoryAnalyzer};
use seen_typechecker::TypeChecker;

/// Simple arena allocator for benchmarking
struct ArenaAllocator {
    buffer: Vec<u8>,
    offset: usize,
}

impl ArenaAllocator {
    fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![0; capacity],
            offset: 0,
        }
    }
    
    fn allocate(&mut self, size: usize, align: usize) -> *mut u8 {
        // Align the offset
        let aligned_offset = (self.offset + align - 1) & !(align - 1);
        
        if aligned_offset + size > self.buffer.capacity() {
            panic!("Arena allocator out of memory");
        }
        
        let ptr = unsafe { self.buffer.as_mut_ptr().add(aligned_offset) };
        self.offset = aligned_offset + size;
        ptr
    }
    
    fn reset(&mut self) {
        self.offset = 0;
    }
}

/// Benchmark custom arena allocator vs default allocator
fn bench_custom_allocators(c: &mut Criterion) {
    let mut group = c.benchmark_group("custom_allocators");
    group.measurement_time(Duration::from_secs(10));
    
    // Test allocation patterns
    let allocation_sizes = vec![8, 16, 32, 64, 128, 256, 512, 1024];
    let num_allocations = 10000;
    
    group.bench_function("arena_allocator", |b| {
        let mut arena = ArenaAllocator::new(10 * 1024 * 1024); // 10MB arena
        
        b.iter_custom(|iters| {
            let start = std::time::Instant::now();
            
            for _ in 0..iters {
                arena.reset();
                
                // Perform many small allocations
                for _ in 0..num_allocations {
                    for &size in &allocation_sizes {
                        let ptr = arena.allocate(size, 8);
                        // Write to memory to ensure it's not optimized away
                        unsafe {
                            std::ptr::write_volatile(ptr, 42);
                        }
                        black_box(ptr);
                    }
                }
            }
            
            start.elapsed()
        });
    });
    
    group.bench_function("default_allocator", |b| {
        b.iter_custom(|iters| {
            let start = std::time::Instant::now();
            
            for _ in 0..iters {
                let mut allocations = Vec::new();
                
                // Perform same allocations with default allocator
                for _ in 0..num_allocations {
                    for &size in &allocation_sizes {
                        let mut vec = vec![0u8; size];
                        vec[0] = 42; // Write to ensure not optimized away
                        allocations.push(vec);
                    }
                }
                
                black_box(allocations);
            }
            
            start.elapsed()
        });
    });
    
    // Verify arena is 30% faster
    group.bench_function("allocator_performance_validation", |b| {
        b.iter_custom(|iters| {
            let mut arena_time = Duration::ZERO;
            let mut default_time = Duration::ZERO;
            
            for _ in 0..iters {
                // Measure arena allocator
                let mut arena = ArenaAllocator::new(10 * 1024 * 1024);
                let start = std::time::Instant::now();
                arena.reset();
                for _ in 0..1000 {
                    for &size in &allocation_sizes {
                        let ptr = arena.allocate(size, 8);
                        unsafe { std::ptr::write_volatile(ptr, 42); }
                    }
                }
                arena_time += start.elapsed();
                
                // Measure default allocator
                let start = std::time::Instant::now();
                let mut vecs = Vec::new();
                for _ in 0..1000 {
                    for &size in &allocation_sizes {
                        vecs.push(vec![42u8; size]);
                    }
                }
                default_time += start.elapsed();
                black_box(vecs);
            }
            
            let speedup = default_time.as_secs_f64() / arena_time.as_secs_f64();
            assert!(
                speedup > 1.30,
                "Arena allocator not fast enough: {:.2}x speedup (need >1.30x)",
                speedup
            );
            
            println!("Arena allocator speedup: {:.2}x", speedup);
            
            arena_time
        });
    });
    
    group.finish();
}

/// Benchmark memory allocation patterns in compiler workloads
fn bench_compiler_allocation_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("compiler_allocations");
    
    // Simulate AST node allocations
    group.bench_function("ast_node_allocations", |b| {
        let mut arena = ArenaAllocator::new(50 * 1024 * 1024); // 50MB
        
        b.iter(|| {
            arena.reset();
            
            // Simulate building an AST with many nodes
            for _ in 0..1000 {
                // Expression nodes (small, frequent)
                for _ in 0..10 {
                    let _expr = arena.allocate(48, 8); // sizeof(Expression)
                }
                
                // Statement nodes (medium)
                for _ in 0..5 {
                    let _stmt = arena.allocate(96, 8); // sizeof(Statement)
                }
                
                // Function nodes (large, less frequent)
                let _func = arena.allocate(256, 8); // sizeof(Function)
            }
        });
    });
    
    // Simulate type checking allocations
    group.bench_function("type_checking_allocations", |b| {
        let mut arena = ArenaAllocator::new(20 * 1024 * 1024); // 20MB
        
        b.iter(|| {
            arena.reset();
            
            // Type inference creates many temporary type variables
            for _ in 0..5000 {
                let _type_var = arena.allocate(32, 8); // Type variable
                let _constraint = arena.allocate(64, 8); // Type constraint
            }
            
            // Substitution maps
            for _ in 0..1000 {
                let _subst_entry = arena.allocate(48, 8);
            }
        });
    });
    
    group.finish();
}

/// Benchmark region-based memory management
fn bench_region_based_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("region_memory");
    
    let type_checker = TypeChecker::new();
    
    group.bench_function("region_allocation", |b| {
        b.iter(|| {
            let mut runtime = RuntimeManager::new();
            
            // Create regions
            runtime.create_region("main").unwrap();
            runtime.create_region("temp").unwrap();
            
            // Allocate objects in regions
            for i in 0..100 {
                let obj = format!("object_{}", i);
                let _ref = runtime.allocate_in_region(obj, "main").unwrap();
            }
            
            // Allocate temporary objects
            for i in 0..500 {
                let temp = vec![i; 100];
                let _ref = runtime.allocate_in_region(temp, "temp").unwrap();
            }
            
            // Deallocate temp region
            runtime.deallocate_region("temp").unwrap();
        });
    });
    
    group.finish();
}

criterion_group!(
    allocator_benchmarks,
    bench_custom_allocators,
    bench_compiler_allocation_patterns,
    bench_region_based_memory
);

criterion_main!(allocator_benchmarks);