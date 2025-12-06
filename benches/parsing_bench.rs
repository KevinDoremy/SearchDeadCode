use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn parsing_benchmark(c: &mut Criterion) {
    // TODO: Add actual parsing benchmarks
    c.bench_function("placeholder", |b| b.iter(|| black_box(1 + 1)));
}

criterion_group!(benches, parsing_benchmark);
criterion_main!(benches);
