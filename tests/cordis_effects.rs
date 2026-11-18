#![cfg(not(target_arch = "wasm32"))]
use i::module::kernel::{Component, Context, EffectScope, ServiceKey};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tokio::sync::{Notify, Semaphore};

#[tokio::test]
async fn dropping_manual_dispose_waiter_does_not_abandon_inverse_or_other_waiters() {
    let scope = EffectScope::new();
    let entered = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let lease = scope
        .acquire({
            let entered = entered.clone();
            let release = release.clone();
            move || async move {
                Ok(move || async move {
                    entered.add_permits(1);
                    release.acquire().await.unwrap().forget();
                    Ok(())
                })
            }
        })
        .await
        .unwrap();
    let waiter = tokio::spawn(async move { lease.dispose().await });
    entered.acquire().await.unwrap().forget();
    waiter.abort();
    let _ = waiter.await;
    release.add_permits(1);
    assert!(tokio::time::timeout(std::time::Duration::from_millis(100), scope.close()).await.is_ok());
}

#[tokio::test]
async fn panicking_disposer_reports_failure_without_abandoning_other_cleanup() {
    let scope = EffectScope::new();
    scope
        .acquire(|| async {
            Ok(|| async {
                panic!("broken disposer");
                #[allow(unreachable_code)]
                Ok(())
            })
        })
        .await
        .unwrap();
    let closed = tokio::time::timeout(std::time::Duration::from_millis(100), scope.close()).await;
    assert!(closed.is_ok(), "panicking inverse abandoned completion");
    assert_eq!(closed.unwrap(), ["effect disposer panicked"]);
}

#[tokio::test]
async fn cancelled_setup_does_not_leave_cleanup_waiting_forever() {
    let scope = EffectScope::new();
    let entered = Arc::new(Semaphore::new(0));
    let setup = tokio::spawn({
        let scope = scope.clone();
        let entered = entered.clone();
        async move {
            scope
                .acquire(|| async move {
                    entered.add_permits(1);
                    std::future::pending::<()>().await;
                    Ok(|| async { Ok(()) })
                })
                .await
        }
    });
    entered.acquire().await.unwrap().forget();
    setup.abort();
    let _ = setup.await;
    let closed = tokio::time::timeout(std::time::Duration::from_millis(100), scope.close()).await;
    assert!(closed.is_ok(), "cancelled setup leaked a pending acquisition");
}

#[tokio::test]
async fn partial_setup_failure_rolls_back_previously_acquired_effects() {
    let root = Context::new();
    let trace = Arc::new(Mutex::new(Vec::new()));
    let fiber = root
        .mount(
            Arc::new(Component::new({
                let trace = trace.clone();
                move |ctx, _| {
                    let trace = trace.clone();
                    async move {
                        ctx.effects()
                            .acquire(|| async move {
                                Ok(move || async move {
                                    trace.lock().unwrap().push("undo");
                                    Ok(())
                                })
                            })
                            .await?;
                        anyhow::bail!("partial setup failed")
                    }
                }
            })),
            json!({}),
        )
        .unwrap();
    assert!(fiber.settled().await.is_err());
    assert_eq!(*trace.lock().unwrap(), ["undo"]);
    fiber.dispose().await.unwrap();
}

#[tokio::test]
async fn top_level_cleanup_starts_reverse_and_drains_concurrently_despite_errors() {
    let scope = EffectScope::new();
    let trace = Arc::new(Mutex::new(Vec::new()));
    let started = Arc::new(Semaphore::new(0));
    let release = Arc::new(Notify::new());
    for name in ["a", "b", "c"] {
        let trace = trace.clone();
        let started = started.clone();
        let release = release.clone();
        scope
            .acquire(|| async move {
                Ok(move || async move {
                    trace.lock().unwrap().push(name);
                    let ready = release.notified();
                    tokio::pin!(ready);
                    ready.as_mut().enable();
                    started.add_permits(1);
                    ready.await;
                    if name == "b" {
                        anyhow::bail!("cleanup b");
                    }
                    Ok(())
                })
            })
            .await
            .unwrap();
    }
    let cleanup = tokio::spawn({
        let scope = scope.clone();
        async move { scope.close().await }
    });
    started.acquire_many(3).await.unwrap().forget();
    assert_eq!(*trace.lock().unwrap(), ["c", "b", "a"]);
    release.notify_waiters();
    assert_eq!(cleanup.await.unwrap(), ["cleanup b"]);
    assert_eq!(scope.close().await, ["cleanup b"]);
}

#[tokio::test]
async fn group_cleanup_is_serial_reverse_and_manual_dispose_is_idempotent() {
    let scope = EffectScope::new();
    let group = scope.group().await.unwrap();
    let trace = Arc::new(Mutex::new(Vec::new()));
    for name in ["a", "b", "c"] {
        let trace = trace.clone();
        group
            .acquire(|| async move {
                Ok(move || async move {
                    trace.lock().unwrap().push(name);
                    tokio::task::yield_now().await;
                    trace.lock().unwrap().push(name);
                    Ok(())
                })
            })
            .await
            .unwrap();
    }
    let disposed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = disposed.clone();
    let lease = scope
        .acquire(|| async move {
            Ok(move || async move {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            })
        })
        .await
        .unwrap();
    lease.dispose().await.unwrap();
    lease.dispose().await.unwrap();
    assert!(scope.close().await.is_empty());
    assert_eq!(*trace.lock().unwrap(), ["c", "c", "b", "b", "a", "a"]);
    assert_eq!(disposed.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn disposer_finishing_setup_after_close_is_not_lost() {
    let scope = EffectScope::new();
    let entered = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let disposed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let pending = tokio::spawn({
        let scope = scope.clone();
        let entered = entered.clone();
        let release = release.clone();
        let disposed = disposed.clone();
        async move {
            scope
                .acquire(|| async move {
                    entered.add_permits(1);
                    release.acquire().await.unwrap().forget();
                    Ok(move || async move {
                        disposed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        Ok(())
                    })
                })
                .await
        }
    });
    entered.acquire().await.unwrap().forget();
    let closing = tokio::spawn({
        let scope = scope.clone();
        async move { scope.close().await }
    });
    while !scope.is_closing() {
        tokio::task::yield_now().await;
    }
    assert!(!closing.is_finished());
    release.add_permits(1);
    assert!(closing.await.unwrap().is_empty());
    pending.await.unwrap().unwrap();
    assert_eq!(disposed.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn service_call_effect_belongs_to_consumer_not_provider() {
    struct Counter(Arc<std::sync::atomic::AtomicUsize>);
    let root = Context::new();
    let value = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    root.provide(ServiceKey::new("counter"), Counter(value.clone())).unwrap();
    let consumer = root
        .mount(
            Arc::new(
                Component::new(|ctx, _| async move {
                    ctx.service(ServiceKey::<Counter>::new("counter"))?
                        .call(|counter, call| {
                            let value = counter.0.clone();
                            async move {
                                call.caller
                                    .effects()
                                    .acquire(|| async move {
                                        value.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                        Ok(move || async move {
                                            value.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                                            Ok(())
                                        })
                                    })
                                    .await
                            }
                        })?
                        .await?;
                    Ok(())
                })
                .requires(ServiceKey::<Counter>::new("counter")),
            ),
            json!({}),
        )
        .unwrap();
    consumer.settled().await.unwrap();
    assert_eq!(value.load(std::sync::atomic::Ordering::SeqCst), 1);
    consumer.dispose().await.unwrap();
    assert_eq!(value.load(std::sync::atomic::Ordering::SeqCst), 0);
}
