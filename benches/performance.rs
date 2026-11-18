//! Production kernel service access and independent Fiber lifecycle.
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use i::module::kernel::{Component, Context, ServiceKey};
use serde_json::Value;
use std::sync::Arc;

fn service_access(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let root = Context::new();
    let key = ServiceKey::<usize>::new("benchmark.value");
    root.provide(key.clone(), 42).unwrap();
    c.bench_function("typed_service_call", |b| {
        b.iter(|| black_box(root.service(key.clone()).unwrap().call(|value, _| *value).unwrap()));
    });
    runtime.block_on(async { root.shutdown().await });
}

fn instances(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("component_instances");
    for count in [10, 100, 1000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter(|| {
                runtime.block_on(async {
                    let root = Context::new();
                    let component = Arc::new(Component::new(|_, _| async { Ok(()) }));
                    let fibers: Vec<_> =
                        (0..count).map(|_| root.mount(component.clone(), Value::Null).unwrap()).collect();
                    for fiber in fibers {
                        fiber.settled().await.unwrap();
                    }
                    assert!(root.shutdown().await.closed);
                })
            });
        });
    }
    group.finish();
}
criterion_group!(benches, service_access, instances);
criterion_main!(benches);
