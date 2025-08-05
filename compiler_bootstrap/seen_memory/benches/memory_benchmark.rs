use criterion::{black_box, criterion_group, criterion_main, Criterion};
use seen_memory::RuntimeManager;

fn benchmark_region_allocation(c: &mut Criterion) {
    c.bench_function("region_allocation", |b| {
        b.iter(|| {
            let mut runtime = RuntimeManager::new();
            for i in 0..1000 {
                let data = format!("test_data_{}", i);
                let _gen_ref = runtime.allocate_in_region(data, "test_region").unwrap();
            }
            runtime.cleanup_region("test_region").unwrap();
        })
    });
}

criterion_group!(benches, benchmark_region_allocation);
criterion_main!(benches);