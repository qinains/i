#![cfg(not(target_arch = "wasm32"))]
use i::module::kernel::{EffectScope, Event, EventValue, ListenOptions};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tokio::sync::Semaphore;

#[tokio::test]
async fn once_and_removed_listeners_release_captured_resources_before_scope_closes() {
    for once in [false, true] {
        let event = Event::<()>::new("resource");
        let scope = EffectScope::new();
        let resource = Arc::new(());
        let weak = Arc::downgrade(&resource);
        let subscription = event
            .on(
                &scope,
                move |_| {
                    let _ = &resource;
                    Ok(EventValue::Undefined)
                },
                ListenOptions { once, ..ListenOptions::default() },
            )
            .unwrap();
        if once {
            event.emit(()).unwrap();
        } else {
            subscription.remove();
        }
        drop(subscription);
        assert!(weak.upgrade().is_none(), "callback resource retained until scope shutdown");
        scope.close().await;
    }
}

#[tokio::test]
async fn bail_and_serial_continue_false_null_undefined_but_stop_on_zero_or_empty_string() {
    for terminal in [json!(0), json!("")] {
        let event = Event::<()>::new("test");
        let scope = EffectScope::new();
        let trace = Arc::new(Mutex::new(Vec::new()));
        for value in [
            EventValue::Undefined,
            EventValue::Value(json!(null)),
            EventValue::Value(json!(false)),
            EventValue::Value(terminal.clone()),
            EventValue::Value(json!("unreachable")),
        ] {
            let trace = trace.clone();
            event
                .on(
                    &scope,
                    move |_| {
                        trace.lock().unwrap().push(value.clone());
                        Ok(value.clone())
                    },
                    ListenOptions::default(),
                )
                .unwrap();
        }
        assert_eq!(event.bail(()).unwrap(), EventValue::Value(terminal.clone()));
        assert_eq!(trace.lock().unwrap().len(), 4);
        trace.lock().unwrap().clear();
        assert_eq!(event.serial(()).await.unwrap(), EventValue::Value(terminal));
        assert_eq!(trace.lock().unwrap().len(), 4);
        assert!(scope.close().await.is_empty());
    }
}

#[tokio::test]
async fn waterfall_is_around_middleware_with_short_circuit_and_single_use_next() {
    let event = Event::<()>::new("around");
    let scope = EffectScope::new();
    let trace = Arc::new(Mutex::new(Vec::new()));
    event
        .around(&scope, {
            let trace = trace.clone();
            move |_, next| {
                let trace = trace.clone();
                async move {
                    trace.lock().unwrap().push("before");
                    let value = next.run().await?;
                    assert!(next.run().await.is_err());
                    trace.lock().unwrap().push("after");
                    Ok(value)
                }
            }
        })
        .unwrap();
    event
        .around(&scope, {
            let trace = trace.clone();
            move |_, _| {
                let trace = trace.clone();
                async move {
                    trace.lock().unwrap().push("stop");
                    Ok(EventValue::Value(json!(7)))
                }
            }
        })
        .unwrap();
    assert_eq!(
        event.waterfall((), || async { panic!("short-circuited tail ran") }).await.unwrap(),
        EventValue::Value(json!(7))
    );
    assert_eq!(*trace.lock().unwrap(), ["before", "stop", "after"]);
    scope.close().await;
}

#[tokio::test]
async fn parallel_waits_for_every_listener_and_collects_independent_failures() {
    let event = Event::<()>::new("parallel");
    let scope = EffectScope::new();
    let entered = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    for name in ["a", "b"] {
        event
            .on_async(&scope, {
                let entered = entered.clone();
                let release = release.clone();
                move |_| {
                    let entered = entered.clone();
                    let release = release.clone();
                    async move {
                        entered.add_permits(1);
                        release.acquire().await.unwrap().forget();
                        anyhow::bail!("{name}")
                    }
                }
            })
            .unwrap();
    }
    let dispatch = tokio::spawn(async move { event.parallel(()).await });
    entered.acquire_many(2).await.unwrap().forget();
    assert!(!dispatch.is_finished());
    release.add_permits(2);
    assert_eq!(dispatch.await.unwrap().unwrap_err().errors, ["a", "b"]);
    scope.close().await;
}

#[tokio::test]
async fn removed_snapshot_listener_is_skipped_and_recursive_once_runs_only_once() {
    let event = Event::<()>::new("dispatch");
    let scope = EffectScope::new();
    let removed = Arc::new(Mutex::new(None::<i::module::kernel::Subscription>));
    let called = Arc::new(Mutex::new(Vec::new()));
    event
        .on(
            &scope,
            {
                let removed = removed.clone();
                move |_| {
                    removed.lock().unwrap().as_ref().unwrap().remove();
                    Ok(EventValue::Undefined)
                }
            },
            ListenOptions::default(),
        )
        .unwrap();
    let second = event
        .on(
            &scope,
            {
                let called = called.clone();
                move |_| {
                    called.lock().unwrap().push("removed");
                    Ok(EventValue::Undefined)
                }
            },
            ListenOptions::default(),
        )
        .unwrap();
    *removed.lock().unwrap() = Some(second);
    event
        .on(
            &scope,
            {
                let event = event.clone();
                let called = called.clone();
                move |_| {
                    called.lock().unwrap().push("once");
                    event.emit(())?;
                    Ok(EventValue::Undefined)
                }
            },
            ListenOptions { once: true, prepend: false },
        )
        .unwrap();
    event.emit(()).unwrap();
    assert_eq!(*called.lock().unwrap(), ["once"]);
    scope.close().await;
}
