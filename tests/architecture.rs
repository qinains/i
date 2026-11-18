//! Executable composition boundaries, not source-string assertions.
use i::module::kernel::{Component, Context, EffectScope, Fiber, ServiceKey};
use serde_json::json;
use std::path::Path;
use std::sync::{Arc, Mutex};

#[test]
fn obsolete_native_composition_entrypoints_are_not_shipped() {
    for path in [
        "src/module/context.rs",
        "src/module/fiber.rs",
        "src/module/plugin.rs",
        "src/module/async_context.rs",
        "src/module/coeffect.rs",
        "src/module/scope.rs",
        "src/module/loader.rs",
        "src/module/examples/mod.rs",
        "src/module/plugins/mod.rs",
        "src/agent/cordis_plugin.rs",
        "src/browser/cordis_plugin.rs",
        "src/workspace/cordis_plugin.rs",
        "src/plugins/mod.rs",
        "src/cli.rs",
    ] {
        assert!(!Path::new(path).exists(), "obsolete composition entrypoint: {path}");
    }
}

#[test]
fn native_kernel_handles_are_send_and_sync() {
    fn check<T: Send + Sync>() {}
    check::<Context>();
    check::<Fiber>();
    check::<EffectScope>();
}

#[tokio::test]
async fn one_definition_owns_independent_instances_and_typed_services() {
    let root = Context::new();
    let key = ServiceKey::<u32>::new("architecture.counter");
    root.provide(key.clone(), 7).unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let values = seen.clone();
    let component = Arc::new(
        Component::new(move |ctx, config| {
            let values = values.clone();
            let key = key.clone();
            async move {
                let value = ctx.service(key)?.call(|value, _| *value)?;
                values.lock().unwrap().push((config, value));
                Ok(())
            }
        })
        .requires(ServiceKey::<u32>::new("architecture.counter")),
    );
    let first = root.mount(component.clone(), json!(1)).unwrap();
    let second = root.mount(component.clone(), json!(2)).unwrap();
    first.settled().await.unwrap();
    second.settled().await.unwrap();
    assert_ne!(first.id(), second.id());
    assert_eq!(root.instances(component.id()).len(), 2);
    let mut values = seen.lock().unwrap().clone();
    values.sort_by_key(|(config, _)| config.as_u64());
    assert_eq!(values, [(json!(1), 7), (json!(2), 7)]);
    assert!(root.shutdown().await.closed);
    assert!(root.mount(component, json!(3)).is_err());
}

#[test]
fn structured_errors_round_trip_without_losing_context() {
    use i::error::{AppError, ErrorCode};
    for code in [ErrorCode::InvalidPath, ErrorCode::PluginLoadFailed, ErrorCode::ConfigInvalid] {
        let error = AppError::with_context(code, "operation failed", "architecture contract");
        let encoded = serde_json::to_value(&error).unwrap();
        assert_eq!(encoded["code"], code.as_u32());
        let restored: AppError = serde_json::from_value(encoded).unwrap();
        assert_eq!(restored.code(), code);
        assert_eq!(restored.message(), "operation failed");
        assert_eq!(restored.context(), Some("architecture contract"));
    }
}
