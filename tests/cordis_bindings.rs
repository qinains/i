#![cfg(not(target_arch = "wasm32"))]
use i::module::kernel::{Component, Context, FiberState, ServiceKey};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tokio::sync::Semaphore;

#[tokio::test]
async fn disposing_consumer_unregisters_before_cleanup_but_still_pins_its_binding() {
    let root = Context::new();
    let binding = root.provide(ServiceKey::<u32>::new("resource"), 7).unwrap();
    let entered = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let component = Arc::new(
        Component::new({
            let entered = entered.clone();
            let release = release.clone();
            move |ctx, _| {
                let entered = entered.clone();
                let release = release.clone();
                async move {
                    ctx.effects()
                        .acquire(|| async move {
                            Ok(move || async move {
                                entered.add_permits(1);
                                release.acquire().await.unwrap().forget();
                                assert_eq!(ctx.service(ServiceKey::<u32>::new("resource"))?.call(|v, _| *v)?, 7);
                                Ok(())
                            })
                        })
                        .await?;
                    Ok(())
                }
            }
        })
        .requires(ServiceKey::<u32>::new("resource")),
    );
    let fiber = root.mount(component.clone(), json!({})).unwrap();
    fiber.settled().await.unwrap();
    let closing = tokio::spawn(async move { fiber.dispose().await });
    entered.acquire().await.unwrap().forget();
    let registered = root.instances(component.id());
    let retiring = tokio::spawn(binding.withdraw());
    tokio::task::yield_now().await;
    let retired_early = retiring.is_finished();
    release.add_permits(1);
    closing.await.unwrap().unwrap();
    retiring.await.unwrap().unwrap();
    assert!(registered.is_empty(), "disposing instance remained publicly registered");
    assert!(!retired_early, "unregistered consumer lost its admitted binding before cleanup");
    root.shutdown().await;
}

#[tokio::test]
async fn provider_own_view_uses_newest_provision_after_manual_replacement() {
    let root = Context::new();
    let provider = root
        .mount(
            Arc::new(Component::new(|ctx, _| async move {
                let lease = ctx.provide(ServiceKey::<u32>::new("own"), 1)?;
                lease.withdraw().await?;
                ctx.provide(ServiceKey::<u32>::new("own"), 2)?;
                assert_eq!(ctx.service(ServiceKey::<u32>::new("own"))?.call(|value, _| *value)?, 2);
                Ok(())
            })),
            json!({}),
        )
        .unwrap();
    provider.settled().await.unwrap();
    root.shutdown().await;
}

#[tokio::test]
async fn dropping_binding_retirement_waiter_does_not_abandon_resource_release() {
    let root = Context::new();
    let value = Arc::new(());
    let weak = Arc::downgrade(&value);
    let binding = root.provide(ServiceKey::new("resource"), value).unwrap();
    let entered = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let consumer = root
        .mount(
            Arc::new(
                Component::new({
                    let entered = entered.clone();
                    let release = release.clone();
                    move |ctx, _| {
                        let entered = entered.clone();
                        let release = release.clone();
                        async move {
                            ctx.effects()
                                .acquire(|| async move {
                                    Ok(move || async move {
                                        entered.add_permits(1);
                                        release.acquire().await.unwrap().forget();
                                        Ok(())
                                    })
                                })
                                .await?;
                            Ok(())
                        }
                    }
                })
                .requires(ServiceKey::<Arc<()>>::new("resource")),
            ),
            json!({}),
        )
        .unwrap();
    consumer.settled().await.unwrap();
    drop(binding.withdraw());
    entered.acquire().await.unwrap().forget();
    assert!(weak.upgrade().is_some());
    release.add_permits(1);
    consumer.settled().await.unwrap();
    let released = tokio::time::timeout(std::time::Duration::from_millis(100), async {
        while weak.upgrade().is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await;
    assert!(released.is_ok(), "retirement cleanup depended on polling the caller's waiter");
    root.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unloading_provider_cannot_publish_a_late_binding_after_retirement_snapshot() {
    let root = Context::new();
    let saved = Arc::new(Mutex::new(None));
    let entered = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let provider = root
        .mount(
            Arc::new(Component::new({
                let saved = saved.clone();
                let entered = entered.clone();
                let release = release.clone();
                move |ctx, _| {
                    let saved = saved.clone();
                    let entered = entered.clone();
                    let release = release.clone();
                    async move {
                        *saved.lock().unwrap() = Some(ctx.clone());
                        ctx.effects()
                            .acquire(|| async move {
                                Ok(move || async move {
                                    entered.add_permits(1);
                                    release.acquire().await.unwrap().forget();
                                    Ok(())
                                })
                            })
                            .await?;
                        Ok(())
                    }
                }
            })),
            json!({}),
        )
        .unwrap();
    provider.settled().await.unwrap();
    let closing = tokio::spawn({
        let provider = provider.clone();
        async move { provider.dispose().await }
    });
    entered.acquire().await.unwrap().forget();
    let published = saved.lock().unwrap().as_ref().unwrap().provide(ServiceKey::<u32>::new("late"), 2);
    release.add_permits(1);
    closing.await.unwrap().unwrap();
    assert!(published.is_err(), "closing activation leaked a late provision");
}

#[tokio::test]
async fn access_retained_after_consumer_disposal_cannot_escape_into_live_services() {
    let root = Context::new();
    root.provide(ServiceKey::<u32>::new("foo"), 1).unwrap();
    let saved = Arc::new(Mutex::new(None));
    let fiber = root
        .mount(
            Arc::new(
                Component::new({
                    let saved = saved.clone();
                    move |ctx, _| {
                        let saved = saved.clone();
                        async move {
                            *saved.lock().unwrap() = Some(ctx.service(ServiceKey::<u32>::new("foo"))?);
                            Ok(())
                        }
                    }
                })
                .requires(ServiceKey::<u32>::new("foo")),
            ),
            json!({}),
        )
        .unwrap();
    fiber.settled().await.unwrap();
    fiber.dispose().await.unwrap();
    assert!(saved.lock().unwrap().as_ref().unwrap().call(|value, _| *value).is_err());
}

#[tokio::test]
async fn definition_site_dependency_is_distinct_from_caller_dependency_and_effect_scope() {
    struct Counter;
    let root = Context::new();
    let foo = ServiceKey::<u32>::new("foo");
    root.provide(foo.clone(), 10).unwrap();
    let isolated = root.isolate(foo.clone(), i::module::kernel::RealmId::new());
    isolated.provide(foo, 20).unwrap();
    let provider = root
        .mount(
            Arc::new(
                Component::new(|ctx, _| async move {
                    ctx.provide(ServiceKey::new("counter"), Counter)?;
                    Ok(())
                })
                .requires(ServiceKey::<u32>::new("foo")),
            ),
            json!({}),
        )
        .unwrap();
    provider.settled().await.unwrap();
    let values = Arc::new(Mutex::new(None));
    let consumer = isolated
        .mount(
            Arc::new(
                Component::new({
                    let values = values.clone();
                    move |ctx, _| {
                        let values = values.clone();
                        async move {
                            let observed = ctx.service(ServiceKey::<Counter>::new("counter"))?.call(|_, call| {
                                let defined =
                                    call.definition.service(ServiceKey::<u32>::new("foo"))?.call(|v, _| *v)?;
                                let called = call.caller.service(ServiceKey::<u32>::new("foo"))?.call(|v, _| *v)?;
                                anyhow::Ok((defined, called))
                            })??;
                            *values.lock().unwrap() = Some(observed);
                            Ok(())
                        }
                    }
                })
                .requires(ServiceKey::<Counter>::new("counter"))
                .requires(ServiceKey::<u32>::new("foo")),
            ),
            json!({}),
        )
        .unwrap();
    consumer.settled().await.unwrap();
    assert_eq!(*values.lock().unwrap(), Some((10, 20)));
    consumer.dispose().await.unwrap();
    provider.dispose().await.unwrap();
}

#[tokio::test]
async fn provider_cleanup_retains_own_provision_during_terminal_shutdown() {
    let root = Context::new();
    let observed = Arc::new(Mutex::new(None));
    let provider = root
        .mount(
            Arc::new(Component::new({
                let observed = observed.clone();
                move |ctx, _| {
                    let observed = observed.clone();
                    async move {
                        ctx.provide(ServiceKey::<u32>::new("own"), 7)?;
                        ctx.effects()
                            .acquire(|| async move {
                                Ok(move || async move {
                                    *observed.lock().unwrap() =
                                        Some(ctx.service(ServiceKey::<u32>::new("own"))?.call(|value, _| *value)?);
                                    Ok(())
                                })
                            })
                            .await?;
                        Ok(())
                    }
                }
            })),
            json!({}),
        )
        .unwrap();
    provider.settled().await.unwrap();
    assert!(root.shutdown().await.errors.is_empty());
    assert_eq!(*observed.lock().unwrap(), Some(7));
}

#[tokio::test]
async fn child_cleanup_can_read_its_parents_own_provision() {
    let root = Context::new();
    let observed = Arc::new(Mutex::new(None));
    let parent = root
        .mount(
            Arc::new(Component::new({
                let observed = observed.clone();
                move |ctx, _| {
                    let observed = observed.clone();
                    async move {
                        ctx.provide(ServiceKey::<u32>::new("own"), 7)?;
                        let child = ctx.context().mount(
                            Arc::new(Component::new(move |ctx, _| {
                                let observed = observed.clone();
                                async move {
                                    ctx.effects()
                                        .acquire(|| async move {
                                            Ok(move || async move {
                                                *observed.lock().unwrap() = Some(
                                                    ctx.service(ServiceKey::<u32>::new("own"))?
                                                        .call(|value, _| *value)?,
                                                );
                                                Ok(())
                                            })
                                        })
                                        .await?;
                                    Ok(())
                                }
                            })),
                            json!({}),
                        )?;
                        child.settled().await?;
                        Ok(())
                    }
                }
            })),
            json!({}),
        )
        .unwrap();
    parent.settled().await.unwrap();
    parent.dispose().await.unwrap();
    assert_eq!(*observed.lock().unwrap(), Some(7));
    assert!(root.shutdown().await.errors.is_empty());
}

#[tokio::test]
async fn child_mount_is_owned_and_inherits_admitted_view_without_inventing_a_realm() {
    let root = Context::new();
    root.provide(ServiceKey::<u32>::new("foo"), 7).unwrap();
    let child_slot = Arc::new(Mutex::new(None));
    let trace = Arc::new(Mutex::new(Vec::new()));
    let parent = root
        .mount(
            Arc::new(
                Component::new({
                    let child_slot = child_slot.clone();
                    let trace = trace.clone();
                    move |ctx, _| {
                        let child_slot = child_slot.clone();
                        let trace = trace.clone();
                        async move {
                            let child = ctx.context().mount(
                                Arc::new(Component::new(move |ctx, _| {
                                    let trace = trace.clone();
                                    async move {
                                        let value =
                                            ctx.service(ServiceKey::<u32>::new("foo"))?.call(|value, _| *value)?;
                                        trace.lock().unwrap().push(value);
                                        Ok(())
                                    }
                                })),
                                json!({}),
                            )?;
                            child.settled().await?;
                            *child_slot.lock().unwrap() = Some(child);
                            Ok(())
                        }
                    }
                })
                .requires(ServiceKey::<u32>::new("foo")),
            ),
            json!({}),
        )
        .unwrap();
    parent.settled().await.unwrap();
    assert_eq!(*trace.lock().unwrap(), [7]);
    parent.dispose().await.unwrap();
    assert_eq!(child_slot.lock().unwrap().as_ref().unwrap().state(), FiberState::Disposed);
}

#[tokio::test]
async fn retired_binding_pins_old_view_until_consumer_cleanup_finishes() {
    let root = Context::new();
    let old = root.provide(ServiceKey::<u32>::new("foo"), 1).unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let entered = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let consumer = root
        .mount(
            Arc::new(
                Component::new({
                    let events = events.clone();
                    let entered = entered.clone();
                    let release = release.clone();
                    move |ctx, _| {
                        let events = events.clone();
                        let entered = entered.clone();
                        let release = release.clone();
                        async move {
                            let value = ctx.service(ServiceKey::<u32>::new("foo"))?.call(|value, _| *value)?;
                            events.lock().unwrap().push(format!("load({value})"));
                            ctx.effects()
                                .acquire(|| async move {
                                    Ok(move || async move {
                                        if value == 1 {
                                            entered.add_permits(1);
                                            release.acquire().await.unwrap().forget();
                                        }
                                        let value =
                                            ctx.service(ServiceKey::<u32>::new("foo"))?.call(|value, _| *value)?;
                                        events.lock().unwrap().push(format!("unload({value})"));
                                        Ok(())
                                    })
                                })
                                .await?;
                            Ok(())
                        }
                    }
                })
                .requires(ServiceKey::<u32>::new("foo")),
            ),
            json!({}),
        )
        .unwrap();
    consumer.settled().await.unwrap();
    let retiring = tokio::spawn(old.withdraw());
    root.provide(ServiceKey::<u32>::new("foo"), 2).unwrap();
    entered.acquire().await.unwrap().forget();
    assert!(!retiring.is_finished());
    release.add_permits(1);
    retiring.await.unwrap().unwrap();
    consumer.settled().await.unwrap();
    assert_eq!(*events.lock().unwrap(), ["load(1)", "unload(1)", "load(2)"]);
    consumer.dispose().await.unwrap();
}

#[tokio::test]
async fn consumers_wait_for_async_provider_readiness_and_drain_before_provider_effects() {
    let root = Context::new();
    let entered = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let events = Arc::new(Mutex::new(Vec::new()));
    let provider = root
        .mount(
            Arc::new(Component::new({
                let entered = entered.clone();
                let release = release.clone();
                let events = events.clone();
                move |ctx, _| {
                    let entered = entered.clone();
                    let release = release.clone();
                    let events = events.clone();
                    async move {
                        ctx.provide(ServiceKey::<u32>::new("foo"), 1)?;
                        ctx.effects()
                            .acquire(|| async move {
                                Ok(move || async move {
                                    events.lock().unwrap().push("provider");
                                    Ok(())
                                })
                            })
                            .await?;
                        entered.add_permits(1);
                        release.acquire().await.unwrap().forget();
                        Ok(())
                    }
                }
            })),
            json!({}),
        )
        .unwrap();
    entered.acquire().await.unwrap().forget();
    let consumer = root
        .mount(
            Arc::new(
                Component::new({
                    let events = events.clone();
                    move |ctx, _| {
                        let events = events.clone();
                        async move {
                            events.lock().unwrap().push("load");
                            ctx.effects()
                                .acquire(|| async move {
                                    Ok(move || async move {
                                        events.lock().unwrap().push("consumer");
                                        Ok(())
                                    })
                                })
                                .await?;
                            Ok(())
                        }
                    }
                })
                .requires(ServiceKey::<u32>::new("foo")),
            ),
            json!({}),
        )
        .unwrap();
    consumer.settled().await.unwrap();
    assert_eq!(consumer.state(), FiberState::Pending);
    assert!(events.lock().unwrap().is_empty());
    release.add_permits(1);
    provider.settled().await.unwrap();
    consumer.settled().await.unwrap();
    assert_eq!(consumer.state(), FiberState::Active);
    provider.dispose().await.unwrap();
    consumer.settled().await.unwrap();
    assert_eq!(*events.lock().unwrap(), ["load", "consumer", "provider"]);
    assert_eq!(consumer.state(), FiberState::Pending);
    consumer.dispose().await.unwrap();
}

#[tokio::test]
async fn sibling_binding_withdrawal_does_not_wait_for_unrelated_consumer() {
    let root = Context::new();
    let a = root.provide(ServiceKey::<u32>::new("a"), 1).unwrap();
    root.provide(ServiceKey::<u32>::new("b"), 2).unwrap();
    let release = Arc::new(Semaphore::new(0));
    let consumer = root
        .mount(
            Arc::new(
                Component::new({
                    let release = release.clone();
                    move |_, _| {
                        let release = release.clone();
                        async move {
                            release.acquire().await.unwrap().forget();
                            Ok(())
                        }
                    }
                })
                .requires(ServiceKey::<u32>::new("b")),
            ),
            json!({}),
        )
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(1), a.withdraw()).await.unwrap().unwrap();
    release.add_permits(1);
    consumer.settled().await.unwrap();
    consumer.dispose().await.unwrap();
}
