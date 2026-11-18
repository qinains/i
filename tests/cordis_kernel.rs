#![cfg(not(target_arch = "wasm32"))]
use i::module::kernel::{Component, Context, FiberState, RealmId, ServiceKey};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn same_definition_mounts_distinct_configured_instances() {
    let root = Context::new();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let trace = seen.clone();
    let component = Arc::new(Component::new(move |_, config| {
        let trace = trace.clone();
        async move {
            trace.lock().unwrap().push(config);
            Ok(())
        }
    }));
    let a = root.mount(component.clone(), json!({"instance": 1})).unwrap();
    let b = root.mount(component, json!({"instance": 2})).unwrap();
    a.settled().await.unwrap();
    b.settled().await.unwrap();
    assert_ne!(a.id(), b.id());
    let mut values = seen.lock().unwrap().clone();
    values.sort_by_key(|v| v["instance"].as_u64());
    assert_eq!(values, [json!({"instance": 1}), json!({"instance": 2})]);
    a.dispose().await.unwrap();
    b.dispose().await.unwrap();
}

#[tokio::test]
async fn independent_missing_dependencies_settle_pending() {
    let root = Context::new();
    let foo = ServiceKey::<u32>::new("foo");
    let a = root
        .mount(Arc::new(Component::new(|_, _| async { panic!("unready component ran") }).requires(foo)), json!({}))
        .unwrap();
    let b = root
        .mount(
            Arc::new(
                Component::new(|_, _| async { panic!("cyclic component ran") }).requires(ServiceKey::<u32>::new("bar")),
            ),
            json!({}),
        )
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(1), a.settled()).await.unwrap().unwrap();
    assert_eq!(a.state(), FiberState::Pending);
    b.settled().await.unwrap();
    assert_eq!(b.state(), FiberState::Pending);
    a.dispose().await.unwrap();
    b.dispose().await.unwrap();
}

#[tokio::test]
async fn isolation_changes_one_key_and_extend_keeps_owner() {
    let root = Context::new();
    let foo = ServiceKey::<u32>::new("foo");
    let bar = ServiceKey::<u32>::new("bar");
    let realm = RealmId::new();
    let isolated = root.isolate(foo.clone(), realm);
    root.provide(foo.clone(), 1).unwrap();
    root.provide(bar.clone(), 7).unwrap();
    isolated.provide(foo.clone(), 2).unwrap();
    let shared = root.isolate(foo.clone(), realm).extend();
    let values = Arc::new(Mutex::new(Vec::new()));
    let seen = values.clone();
    let component = Arc::new(
        Component::new(move |ctx, _| {
            let values = seen.clone();
            let foo = foo.clone();
            let bar = bar.clone();
            async move {
                let a = ctx.service(foo)?.call(|value, _| *value)?;
                let b = ctx.service(bar)?.call(|value, _| *value)?;
                values.lock().unwrap().push((a, b));
                Ok(())
            }
        })
        .requires(ServiceKey::<u32>::new("foo"))
        .requires(ServiceKey::<u32>::new("bar")),
    );
    let a = root.mount(component.clone(), json!({})).unwrap();
    let b = shared.mount(component, json!({})).unwrap();
    a.settled().await.unwrap();
    b.settled().await.unwrap();
    let mut observed = values.lock().unwrap().clone();
    observed.sort();
    assert_eq!(observed, [(1, 7), (2, 7)]);
    assert_eq!(root.owner_id(), root.extend().owner_id());
    a.dispose().await.unwrap();
    b.dispose().await.unwrap();
}
