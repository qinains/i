use anyhow::{Context as _, Result};
use i::module::kernel::{Component, ServiceCall, ServiceKey};
use serde_json::Value;
use std::sync::{Arc, Mutex};

pub struct MetadataService {
    pub dependency: Option<String>,
}
impl MetadataService {
    fn tags(&self, call: ServiceCall) -> Result<Vec<String>> {
        if let Some(dependency) = &self.dependency {
            call.service(ServiceKey::<Self>::new(dependency.clone()))?.call(|service, call| service.tags(call))?
        } else {
            call.metadata::<Vec<String>, _>(|layers| layers.iter().flat_map(|tags| tags.iter().cloned()).collect())
        }
    }
}

pub fn definition(spec: &Value, trace: Arc<Mutex<Vec<String>>>) -> Result<Option<Arc<Component>>> {
    let kind = spec["kind"].as_str().unwrap_or_default();
    if !matches!(kind, "metadata" | "facade") {
        return Ok(None);
    }
    let facade = kind == "facade";
    let spec = spec.clone();
    let declaration = spec.clone();
    let mut component = Component::new(move |ctx, _| {
        let trace = trace.clone();
        let spec = spec.clone();
        async move {
            let read = spec["read"].as_str().context("missing read")?.to_owned();
            if facade {
                let name = spec["provide"].as_str().context("missing provide")?.to_owned();
                ctx.provide(ServiceKey::new(name), MetadataService { dependency: Some(read) })?;
            } else {
                let mut context = ctx.context();
                if let Some(layers) = spec["use_tags"].as_object() {
                    for (service, value) in layers {
                        let tags: Vec<String> = serde_json::from_value(value.clone())?;
                        context = context.intercept(ServiceKey::<MetadataService>::new(service.clone()), tags);
                    }
                }
                let tags = context
                    .service(ServiceKey::<MetadataService>::new(read))?
                    .call(|service, call| service.tags(call))??;
                trace.lock().unwrap().push(format!("tags({})", tags.join(",")));
            }
            Ok(())
        }
    });
    for dependency in declaration["inject"].as_array().context("missing inject")? {
        let name = dependency.as_str().context("invalid dependency")?;
        let key = ServiceKey::<MetadataService>::new(name.to_owned());
        component = if let Some(tags) = declaration["inject_tags"].get(name) {
            component.requires_with(key, serde_json::from_value::<Vec<String>>(tags.clone())?)
        } else {
            component.requires(key)
        };
    }
    Ok(Some(Arc::new(component)))
}
