#![cfg(not(target_arch = "wasm32"))]
use i::module::kernel::{Component, Context, FiberState, ServiceKey};
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Semaphore;

#[tokio::test]
async fn manually_disposed_instance_is_not_kept_alive_by_its_root_effect() {
    let root = Context::new();
    let capture = Arc::new(());
    let weak = Arc::downgrade(&capture);
    let component = Arc::new(Component::new(move |_, _| {
        let capture = capture.clone();
        async move {
            drop(capture);
            Ok(())
        }
    }));
    let fiber = root.mount(component, json!({})).unwrap();
    fiber.settled().await.unwrap();
    fiber.dispose().await.unwrap();
    drop(fiber);
    let dropped = tokio::time::timeout(Duration::from_millis(100), async {
        while weak.upgrade().is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await;
    assert!(dropped.is_ok(), "root ownership retained a disposed component instance");
    root.shutdown().await;
}

#[tokio::test]
async fn terminal_shutdown_wins_over_an_in_flight_root_restart() {
    let root = Context::new();
    let entered = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    root.effects()
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
    let restarting = root.restart();
    entered.acquire().await.unwrap().forget();
    let shutdown = root.shutdown();
    release.add_permits(1);
    assert!(restarting.await.is_err(), "restart reported success after terminal closure won");
    assert!(shutdown.await.closed);
    assert!(root.provide(ServiceKey::<u32>::new("late"), 1).is_err());
}

#[tokio::test]
async fn retired_binding_lease_does_not_retain_released_provider_resources() {
    let root = Context::new();
    let value = Arc::new(());
    let weak = Arc::downgrade(&value);
    let lease = root.provide(ServiceKey::new("resource"), value).unwrap();
    lease.withdraw().await.unwrap();
    assert!(weak.upgrade().is_none(), "retired binding retained its service value");
    assert!(root.shutdown().await.closed);
}

#[tokio::test]
async fn root_restart_releases_owned_bindings_children_and_allows_reuse() {
    let root = Context::new();
    let cleaned = Arc::new(Mutex::new(Vec::new()));
    root.provide(ServiceKey::<u32>::new("foo"), 1).unwrap();
    let child = root
        .mount(
            Arc::new(
                Component::new({
                    let cleaned = cleaned.clone();
                    move |ctx, _| {
                        let cleaned = cleaned.clone();
                        async move {
                            ctx.effects()
                                .acquire(|| async move {
                                    Ok(move || async move {
                                        cleaned.lock().unwrap().push("child");
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
    child.settled().await.unwrap();
    root.restart().await.unwrap();
    assert_eq!(child.state(), FiberState::Disposed);
    assert_eq!(*cleaned.lock().unwrap(), ["child"]);
    assert!(root.service(ServiceKey::<u32>::new("foo")).is_err());
    root.provide(ServiceKey::<u32>::new("foo"), 2).unwrap();
    let later = root
        .mount(
            Arc::new(
                Component::new(|ctx, _| async move {
                    assert_eq!(ctx.service(ServiceKey::<u32>::new("foo"))?.call(|v, _| *v)?, 2);
                    Ok(())
                })
                .requires(ServiceKey::<u32>::new("foo")),
            ),
            json!({}),
        )
        .unwrap();
    later.settled().await.unwrap();
    assert!(root.shutdown().await.closed);
}

#[tokio::test]
async fn terminal_shutdown_joins_rejects_new_work_and_aggregates_cleanup_errors() {
    let root = Context::new();
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    for fail in [false, true] {
        let count = count.clone();
        root.effects()
            .acquire(|| async move {
                Ok(move || async move {
                    count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    anyhow::ensure!(!fail, "cleanup failed");
                    Ok(())
                })
            })
            .await
            .unwrap();
    }
    let first = root.shutdown();
    let second = root.shutdown();
    assert!(root.provide(ServiceKey::<u32>::new("late"), 3).is_err());
    assert!(root.mount(Arc::new(Component::new(|_, _| async { Ok(()) })), json!({})).is_err());
    let (first, second) = tokio::join!(first, second);
    assert_eq!(first, second);
    assert!(first.closed);
    assert_eq!(first.errors, ["cleanup failed"]);
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert!(root.restart().await.is_err());
    assert_eq!(root.shutdown().await, first);
}

#[tokio::test]
async fn shutdown_deadline_reports_retained_resource_without_freeing_it_early() {
    let root = Context::new();
    let entered = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let freed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    root.effects()
        .acquire({
            let entered = entered.clone();
            let release = release.clone();
            let freed = freed.clone();
            move || async move {
                Ok(move || async move {
                    entered.add_permits(1);
                    release.acquire().await.unwrap().forget();
                    freed.store(true, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                })
            }
        })
        .await
        .unwrap();
    let closing = root.shutdown_with_timeout(Duration::from_millis(50));
    entered.acquire().await.unwrap().forget();
    let report = closing.await;
    assert!(!report.closed);
    assert!(!report.retained.is_empty());
    assert!(!freed.load(std::sync::atomic::Ordering::SeqCst));
    release.add_permits(1);
    tokio::time::timeout(Duration::from_secs(1), async {
        while !freed.load(std::sync::atomic::Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn hook_can_request_ancestor_shutdown_without_waiting_for_its_own_teardown() {
    let root = Context::new();
    let child = root
        .mount(
            Arc::new(Component::new(|ctx, _| async move {
                ctx.context().request_shutdown();
                Ok(())
            })),
            json!({}),
        )
        .unwrap();
    child.settled().await.unwrap();
    let report = root.shutdown().await;
    assert!(report.closed);
    assert_eq!(child.state(), FiberState::Disposed);
}

#[tokio::test]
async fn dependency_withdrawal_cancels_loading_work_and_drains_its_safe_step() {
    let root = Context::new();
    let binding = root.provide(ServiceKey::<u32>::new("foo"), 1).unwrap();
    let entered = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let task = root
        .mount(
            Arc::new(
                Component::new({
                    let entered = entered.clone();
                    let release = release.clone();
                    move |ctx, _| {
                        let entered = entered.clone();
                        let release = release.clone();
                        async move {
                            entered.add_permits(1);
                            ctx.cancellation().cancelled().await;
                            release.acquire().await.unwrap().forget();
                            Ok(())
                        }
                    }
                })
                .requires(ServiceKey::<u32>::new("foo")),
            ),
            json!({}),
        )
        .unwrap();
    entered.acquire().await.unwrap().forget();
    let retiring = tokio::spawn(binding.withdraw());
    tokio::task::yield_now().await;
    assert!(!retiring.is_finished());
    release.add_permits(1);
    tokio::time::timeout(Duration::from_secs(1), retiring).await.unwrap().unwrap().unwrap();
    assert_eq!(task.settled().await.unwrap(), FiberState::Pending);
    root.shutdown().await;
}
