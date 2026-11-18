#![cfg(not(target_arch = "wasm32"))]

use anyhow::Result;
use i::module::kernel::{Component, Context, ServiceCall, ServiceKey};
use serde_json::json;
use std::sync::{Arc, Mutex};

struct Tagged;
#[derive(Clone)]
struct Tags(Vec<&'static str>);

fn tags(call: ServiceCall) -> Result<Vec<&'static str>> {
    call.metadata::<Tags, _>(|layers| layers.iter().flat_map(|tags| tags.0.iter().copied()).collect())
}

#[tokio::test]
async fn metadata_merges_declared_then_outer_to_inner_without_creating_an_owner() {
    let root = Context::new();
    let key = ServiceKey::<Tagged>::new("tagged");
    root.provide(key.clone(), Tagged).unwrap();
    let outer = root.intercept(key.clone(), Tags(vec!["outer"]));
    let observed = Arc::new(Mutex::new(Vec::new()));
    let fiber = outer
        .mount(
            Arc::new(
                Component::new({
                    let observed = observed.clone();
                    let key = key.clone();
                    move |ctx, _| {
                        let observed = observed.clone();
                        let key = key.clone();
                        async move {
                            let inner = ctx.context().intercept(key.clone(), Tags(vec!["inner"]));
                            assert_eq!(inner.owner_id(), ctx.context().owner_id());
                            *observed.lock().unwrap() = inner.service(key)?.call(|_, call| tags(call))??;
                            Ok(())
                        }
                    }
                })
                .requires_with(key.clone(), Tags(vec!["inject"])),
            ),
            json!({}),
        )
        .unwrap();
    fiber.settled().await.unwrap();
    assert_eq!(*observed.lock().unwrap(), ["inject", "outer", "inner"]);
    assert!(root.service(key.clone()).unwrap().call(|_, call| tags(call)).unwrap().unwrap().is_empty());
    assert!(root.intercept(key.clone(), 42_u32).service(key).unwrap().call(|_, call| tags(call)).unwrap().is_err());
    root.shutdown().await;
}

#[tokio::test]
async fn nested_service_call_keeps_definition_injection_but_uses_caller_interception() {
    struct Facade;
    let root = Context::new();
    let tagged = ServiceKey::<Tagged>::new("tagged");
    root.provide(tagged.clone(), Tagged).unwrap();
    let provider = root
        .mount(
            Arc::new(
                Component::new(|ctx, _| async move {
                    ctx.provide(ServiceKey::new("facade"), Facade)?;
                    Ok(())
                })
                .requires_with(tagged.clone(), Tags(vec!["definition"])),
            ),
            json!({}),
        )
        .unwrap();
    provider.settled().await.unwrap();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let fiber = root
        .intercept(tagged.clone(), Tags(vec!["caller"]))
        .mount(
            Arc::new(
                Component::new({
                    let observed = observed.clone();
                    move |ctx, _| {
                        let observed = observed.clone();
                        async move {
                            let value = ctx.service(ServiceKey::<Facade>::new("facade"))?.call(|_, call| {
                                call.service(ServiceKey::<Tagged>::new("tagged"))?.call(|_, call| tags(call))?
                            })??;
                            *observed.lock().unwrap() = value;
                            Ok(())
                        }
                    }
                })
                .requires(ServiceKey::<Facade>::new("facade"))
                .requires_with(tagged, Tags(vec!["wrong-caller-injection"])),
            ),
            json!({}),
        )
        .unwrap();
    fiber.settled().await.unwrap();
    assert_eq!(*observed.lock().unwrap(), ["definition", "caller"]);
    root.shutdown().await;
}

#[tokio::test]
async fn nearest_injection_wins_and_own_provision_does_not_inherit_dependency_metadata() {
    let root = Context::new();
    let key = ServiceKey::<Tagged>::new("tagged");
    root.provide(key.clone(), Tagged).unwrap();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let parent = root
        .mount(
            Arc::new(
                Component::new({
                    let observed = observed.clone();
                    let key = key.clone();
                    move |ctx, _| {
                        let observed = observed.clone();
                        let key = key.clone();
                        async move {
                            for own_declaration in [false, true] {
                                let child = Component::new({
                                    let observed = observed.clone();
                                    let key = key.clone();
                                    move |ctx, _| {
                                        let observed = observed.clone();
                                        let key = key.clone();
                                        async move {
                                            let value = ctx.service(key)?.call(|_, call| tags(call))??;
                                            observed.lock().unwrap().push(value);
                                            Ok(())
                                        }
                                    }
                                });
                                let child = if own_declaration {
                                    child.requires_with(key.clone(), Tags(vec!["nearest"]))
                                } else {
                                    child
                                };
                                ctx.context().mount(Arc::new(child), json!({}))?.settled().await?;
                            }
                            let isolated = ctx.context().isolate(key.clone(), i::module::kernel::RealmId::new());
                            isolated.provide(key.clone(), Tagged)?;
                            observed.lock().unwrap().push(isolated.service(key)?.call(|_, call| tags(call))??);
                            Ok(())
                        }
                    }
                })
                .requires_with(key, Tags(vec!["outer"])),
            ),
            json!({}),
        )
        .unwrap();
    parent.settled().await.unwrap();
    assert_eq!(*observed.lock().unwrap(), [vec!["outer"], vec!["nearest"], vec![]]);
    root.shutdown().await;
}
