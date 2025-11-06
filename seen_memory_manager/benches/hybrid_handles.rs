use criterion::{black_box, criterion_group, criterion_main, Criterion};
use seen_memory_manager::handles::HybridGenerationalArena;

fn hot_lookup_bench(c: &mut Criterion) {
    let mut arena = HybridGenerationalArena::new();
    let handle = arena.insert(42u64);

    c.bench_function("resolve_checked", |b| {
        b.iter(|| {
            let value = arena.resolve(handle).expect("handle should stay live");
            black_box(*value);
        });
    });

    c.bench_function("resolve_fast", |b| {
        b.iter(|| {
            let value = arena
                .resolve_fast(handle)
                .expect("fast path should mirror checked path");
            black_box(*value);
        });
    });

    c.bench_function("resolve_unchecked", |b| {
        b.iter(|| unsafe {
            let value = arena.resolve_unchecked(handle);
            black_box(*value);
        });
    });

    // prevent compiler from discarding the arena entirely
    black_box(handle);
}

criterion_group!(hybrid_handles, hot_lookup_bench);
criterion_main!(hybrid_handles);
