//! Focused benchmarks of the production native effect runner.
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use i::module::kernel::EffectScope;

fn effects(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    c.bench_function("effect_acquire_dispose", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let scope = EffectScope::new();
                let lease = scope.acquire(|| async { Ok(|| async { Ok(()) }) }).await.unwrap();
                lease.dispose().await.unwrap();
                black_box(scope.close().await);
            })
        });
    });
    c.bench_function("effect_group_cleanup", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let scope = EffectScope::new();
                let group = scope.group().await.unwrap();
                for _ in 0..10 {
                    group.acquire(|| async { Ok(|| async { Ok(()) }) }).await.unwrap();
                }
                black_box(scope.close().await);
            })
        });
    });
}
criterion_group!(benches, effects);
criterion_main!(benches);
