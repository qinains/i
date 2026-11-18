#![cfg(not(target_arch = "wasm32"))]
use i::module::kernel::{Component, Context, FiberState, ServiceKey};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tokio::sync::Semaphore;

#[tokio::test]
async fn failed_superseded_setup_cannot_poison_the_new_target() {
    let root = Context::new();
    let entered = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let fiber = root
        .mount(
            Arc::new(Component::new({
                let entered = entered.clone();
                let release = release.clone();
                move |_, config| {
                    let entered = entered.clone();
                    let release = release.clone();
                    async move {
                        if config["old"] == true {
                            entered.add_permits(1);
                            release.acquire().await.unwrap().forget();
                            anyhow::bail!("obsolete failure");
                        }
                        Ok(())
                    }
                }
            })),
            json!({"old": true}),
        )
        .unwrap();
    entered.acquire().await.unwrap().forget();
    fiber.update(json!({"old": false})).unwrap();
    release.add_permits(1);
    assert_eq!(fiber.settled().await.unwrap(), FiberState::Active);
    fiber.dispose().await.unwrap();
}

#[tokio::test]
async fn panicking_hook_rolls_back_and_settles_failed_instead_of_orphaning_worker() {
    let root = Context::new();
    let fiber = root
        .mount(
            Arc::new(Component::new(|_, _| async {
                panic!("broken hook");
                #[allow(unreachable_code)]
                Ok(())
            })),
            json!({}),
        )
        .unwrap();
    let settled = tokio::time::timeout(std::time::Duration::from_millis(100), fiber.settled()).await;
    assert!(settled.is_ok(), "panic orphaned the Fiber transition");
    assert!(settled.unwrap().is_err());
    assert_eq!(fiber.state(), FiberState::Failed);
    fiber.dispose().await.unwrap();
}

#[tokio::test]
async fn failed_component_stays_failed_after_refresh_until_explicit_update() {
    let root = Context::new();
    let binding = root.provide(ServiceKey::<u32>::new("foo"), 1).unwrap();
    let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let component = Arc::new(
        Component::new({
            let attempts = attempts.clone();
            move |_, config| {
                let attempts = attempts.clone();
                async move {
                    attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    anyhow::ensure!(config["recover"] == true, "setup failed");
                    Ok(())
                }
            }
        })
        .requires(ServiceKey::<u32>::new("foo")),
    );
    let fiber = root.mount(component, json!({})).unwrap();
    assert!(fiber.settled().await.is_err());
    let retiring = binding.withdraw();
    root.provide(ServiceKey::<u32>::new("foo"), 2).unwrap();
    retiring.await.unwrap();
    assert!(fiber.settled().await.is_err());
    assert_eq!(fiber.state(), FiberState::Failed);
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);
    fiber.update(json!({"recover": true})).unwrap();
    fiber.settled().await.unwrap();
    assert_eq!(fiber.state(), FiberState::Active);
    fiber.dispose().await.unwrap();
}

#[tokio::test]
async fn independent_fiber_progresses_and_superseded_load_rolls_back_to_latest_config() {
    let root = Context::new();
    let entered = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let trace = Arc::new(Mutex::new(Vec::new()));
    let a = root
        .mount(
            Arc::new(Component::new({
                let entered = entered.clone();
                let release = release.clone();
                let trace = trace.clone();
                move |ctx, config| {
                    let entered = entered.clone();
                    let release = release.clone();
                    let trace = trace.clone();
                    async move {
                        let version = config["version"].as_u64().unwrap();
                        trace.lock().unwrap().push(format!("load({version})"));
                        ctx.effects()
                            .acquire({
                                let trace = trace.clone();
                                move || async move {
                                    Ok(move || async move {
                                        trace.lock().unwrap().push(format!("undo({version})"));
                                        Ok(())
                                    })
                                }
                            })
                            .await?;
                        if version == 1 {
                            entered.add_permits(1);
                            release.acquire().await.unwrap().forget();
                        }
                        Ok(())
                    }
                }
            })),
            json!({"version": 1}),
        )
        .unwrap();
    entered.acquire().await.unwrap().forget();
    let b = root.mount(Arc::new(Component::new(|_, _| async { Ok(()) })), json!({})).unwrap();
    b.settled().await.unwrap();
    assert_eq!(b.state(), FiberState::Active);
    a.update(json!({"version": 2})).unwrap();
    a.update(json!({"version": 3})).unwrap();
    release.add_permits(1);
    a.settled().await.unwrap();
    assert_eq!(*trace.lock().unwrap(), ["load(1)", "undo(1)", "load(3)"]);
    a.dispose().await.unwrap();
    b.dispose().await.unwrap();
}

#[tokio::test]
async fn update_during_unload_does_not_run_obsolete_config() {
    let root = Context::new();
    let entered = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let trace = Arc::new(Mutex::new(Vec::new()));
    let fiber = root
        .mount(
            Arc::new(Component::new({
                let entered = entered.clone();
                let release = release.clone();
                let trace = trace.clone();
                move |ctx, config| {
                    let entered = entered.clone();
                    let release = release.clone();
                    let trace = trace.clone();
                    async move {
                        let version = config["version"].as_u64().unwrap();
                        trace.lock().unwrap().push(version);
                        ctx.effects()
                            .acquire(|| async move {
                                Ok(move || async move {
                                    if version == 1 {
                                        entered.add_permits(1);
                                        release.acquire().await.unwrap().forget();
                                    }
                                    Ok(())
                                })
                            })
                            .await?;
                        Ok(())
                    }
                }
            })),
            json!({"version": 1}),
        )
        .unwrap();
    fiber.settled().await.unwrap();
    fiber.update(json!({"version": 2})).unwrap();
    entered.acquire().await.unwrap().forget();
    fiber.update(json!({"version": 3})).unwrap();
    release.add_permits(1);
    fiber.settled().await.unwrap();
    assert_eq!(*trace.lock().unwrap(), [1, 3]);
    fiber.dispose().await.unwrap();
}

#[tokio::test]
async fn definition_replacement_preserves_instance_identity_config_and_forwards_future_mounts() {
    let root = Context::new();
    let trace = Arc::new(Mutex::new(Vec::new()));
    let old = Arc::new(Component::new(|_, _| async { Ok(()) }));
    let fiber = root.mount(old.clone(), json!({"config": 7})).unwrap();
    fiber.settled().await.unwrap();
    let id = fiber.id();
    let new = Arc::new(
        Component::new({
            let trace = trace.clone();
            move |ctx, config| {
                let trace = trace.clone();
                async move {
                    let value = ctx.service(ServiceKey::<u32>::new("foo"))?.call(|value, _| *value)?;
                    trace.lock().unwrap().push((config["config"].as_u64().unwrap(), value));
                    Ok(())
                }
            }
        })
        .requires(ServiceKey::<u32>::new("foo")),
    );
    root.replace_component(old.id(), new).unwrap();
    fiber.settled().await.unwrap();
    assert_eq!(fiber.id(), id);
    assert_eq!(fiber.state(), FiberState::Pending);
    root.provide(ServiceKey::<u32>::new("foo"), 9).unwrap();
    fiber.settled().await.unwrap();
    assert_eq!(*trace.lock().unwrap(), [(7, 9)]);
    let later = root.mount(old, json!({"config": 8})).unwrap();
    later.settled().await.unwrap();
    assert_eq!(*trace.lock().unwrap(), [(7, 9), (8, 9)]);
    fiber.dispose().await.unwrap();
    later.dispose().await.unwrap();
}
