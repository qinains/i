use anyhow::{Context as _, Result, bail, ensure};
use i::module::kernel::{BindingLease, Component, Context, Fiber, FiberState, RealmId, ServiceCall, ServiceKey};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

#[path = "cordis_events.rs"]
mod event_scenarios;
#[path = "cordis_metadata.rs"]
mod metadata;

struct Counter {
    count: Arc<AtomicUsize>,
    events: Arc<Mutex<Vec<String>>>,
}
impl Counter {
    async fn increase(&self, call: ServiceCall) -> Result<()> {
        let group = call.caller.effects().group().await?;
        for name in ["first", "second"] {
            let count = self.count.clone();
            let events = self.events.clone();
            group
                .acquire(|| async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Ok(move || async move {
                        count.fetch_sub(1, Ordering::SeqCst);
                        events.lock().unwrap().push(format!("release({name})"));
                        Ok(())
                    })
                })
                .await?;
        }
        Ok(())
    }
}

struct Gate {
    entered: watch::Sender<bool>,
    opened: watch::Sender<bool>,
}

impl Gate {
    fn new() -> Self {
        Self { entered: watch::channel(false).0, opened: watch::channel(false).0 }
    }
    async fn wait(&self, name: &str, events: &Mutex<Vec<String>>) -> Result<()> {
        events.lock().unwrap().push(format!("gate({name})"));
        self.entered.send_replace(true);
        self.opened.subscribe().wait_for(|open| *open).await?;
        Ok(())
    }
}

#[derive(serde::Serialize)]
pub struct Outcome {
    pub events: Vec<String>,
    pub states: HashMap<String, String>,
    pub errors: Vec<String>,
}

fn state_name(state: FiberState) -> &'static str {
    match state {
        FiberState::Pending => "PENDING",
        FiberState::Loading => "LOADING",
        FiberState::Active => "ACTIVE",
        FiberState::Failed => "FAILED",
        FiberState::Unloading => "UNLOADING",
        FiberState::Disposed => "DISPOSED",
    }
}

fn definition(
    spec: &Value,
    events: Arc<Mutex<Vec<String>>>,
    gates: Arc<HashMap<String, Gate>>,
) -> Result<Arc<Component>> {
    if let Some(component) = metadata::definition(spec, events.clone())? {
        return Ok(component);
    }
    let field = |name: &str| spec[name].as_str().map(str::to_owned).with_context(|| format!("missing {name}"));
    let read = field("read")?;
    let load = field("load")?;
    let unload = field("unload")?;
    if spec["kind"] == "counter" {
        let mut component = Component::new(move |ctx, _| {
            let events = events.clone();
            let read = read.clone();
            let load = load.clone();
            let unload = unload.clone();
            async move {
                events.lock().unwrap().push(format!("{load}()"));
                ctx.service(ServiceKey::<Counter>::new(read))?
                    .call_async(|counter, call| async move { counter.increase(call).await })
                    .await?;
                ctx.effects()
                    .acquire(|| async move {
                        Ok(move || async move {
                            events.lock().unwrap().push(format!("{unload}()"));
                            Ok(())
                        })
                    })
                    .await?;
                Ok(())
            }
        });
        for dependency in spec["inject"].as_array().context("missing inject")? {
            component = component
                .requires(ServiceKey::<Counter>::new(dependency.as_str().context("invalid dependency")?.to_owned()));
        }
        return Ok(Arc::new(component));
    }
    let load_gate = spec["load_gate"].as_str().map(str::to_owned);
    let unload_gate = spec["unload_gate"].as_str().map(str::to_owned);
    let ready_gate = spec["ready_gate"].as_str().map(str::to_owned);
    let provide = spec["provide"].as_str().map(str::to_owned);
    let config_label = spec["config_label"].as_str().map(str::to_owned);
    let fail_versions = spec["fail_versions"].as_array().cloned().unwrap_or_default();
    let mut component = Component::new(move |ctx, config| {
        let trace = events.clone();
        let read = read.clone();
        let load = load.clone();
        let unload = unload.clone();
        let gates = gates.clone();
        let load_gate = load_gate.clone();
        let unload_gate = unload_gate.clone();
        let ready_gate = ready_gate.clone();
        let provide = provide.clone();
        let config_value = config_label.as_ref().map(|field| config[field].clone());
        let fail_versions = fail_versions.clone();
        async move {
            if let Some(name) = load_gate {
                gates.get(&name).context("unknown gate")?.wait(&name, &trace).await?;
            }
            let version =
                ctx.service(ServiceKey::<Value>::new(read.clone()))?.call(|value, _| value["version"].clone())?;
            trace.lock().unwrap().push(format!("{load}({})", config_value.as_ref().unwrap_or(&version)));
            let fail = fail_versions.contains(&version);
            if let Some(name) = provide {
                ctx.provide(ServiceKey::<Value>::new(name), serde_json::json!({"version": version}))?;
            }
            let ready_gates = gates.clone();
            let ready_trace = trace.clone();
            ctx.effects()
                .acquire(|| async move {
                    Ok(move || async move {
                        if let Some(name) = unload_gate {
                            gates.get(&name).context("unknown gate")?.wait(&name, &trace).await?;
                        }
                        let version = match config_value {
                            Some(value) => value,
                            None => ctx
                                .service(ServiceKey::<Value>::new(read))?
                                .call(|value, _| value["version"].clone())?,
                        };
                        trace.lock().unwrap().push(format!("{unload}({version})"));
                        Ok(())
                    })
                })
                .await?;
            if let Some(name) = ready_gate {
                ready_gates.get(&name).context("unknown gate")?.wait(&name, &ready_trace).await?;
            }
            ensure!(!fail, "activation-failed");
            Ok(())
        }
    });
    for dependency in spec["inject"].as_array().context("missing inject")? {
        component =
            component.requires(ServiceKey::<Value>::new(dependency.as_str().context("invalid dependency")?.to_owned()));
    }
    Ok(Arc::new(component))
}

fn validate(scenario: &Value) -> Result<()> {
    ensure!(scenario["version"] == 1, "unsupported scenario version");
    let mut gates = HashSet::new();
    for gate in scenario["gates"].as_array().context("missing gates")? {
        ensure!(gates.insert(gate.as_str().context("invalid gate")?), "duplicate gate");
    }
    for definition in scenario["definitions"].as_object().context("missing definitions")?.values() {
        for key in ["load_gate", "unload_gate", "ready_gate"] {
            if let Some(value) = definition.get(key) {
                ensure!(gates.contains(value.as_str().context("invalid gate reference")?), "unknown gate");
            }
        }
    }
    for action in scenario["actions"].as_array().context("missing actions")? {
        if let Some(value) = action.get("gate") {
            ensure!(gates.contains(value.as_str().context("invalid gate reference")?), "unknown gate");
        }
    }
    let mut edges = HashMap::<&str, Vec<&str>>::new();
    let mut degree = HashMap::<&str, usize>::new();
    for pair in scenario["before"].as_array().context("missing ordering")? {
        let pair = pair.as_array().context("invalid ordering pair")?;
        ensure!(pair.len() == 2, "invalid ordering pair");
        let a = pair[0].as_str().context("invalid ordering event")?;
        let b = pair[1].as_str().context("invalid ordering event")?;
        edges.entry(a).or_default().push(b);
        degree.entry(a).or_default();
        *degree.entry(b).or_default() += 1;
    }
    let mut ready: Vec<_> = degree.iter().filter_map(|(name, degree)| (*degree == 0).then_some(*name)).collect();
    let mut visited = 0;
    while let Some(name) = ready.pop() {
        visited += 1;
        for child in edges.get(name).into_iter().flatten() {
            let degree = degree.get_mut(child).unwrap();
            *degree -= 1;
            if *degree == 0 {
                ready.push(child);
            }
        }
    }
    ensure!(visited == degree.len(), "ordering cycle");
    Ok(())
}

pub async fn run_scenario(scenario: &Value) -> Result<Outcome> {
    validate(scenario)?;
    let root = Context::new();
    let events = Arc::new(Mutex::new(Vec::new()));
    let gates = Arc::new(
        scenario["gates"]
            .as_array()
            .context("missing gates")?
            .iter()
            .map(|name| Ok((name.as_str().context("invalid gate")?.to_owned(), Gate::new())))
            .collect::<Result<HashMap<_, _>>>()?,
    );
    let definitions = scenario["definitions"]
        .as_object()
        .context("missing definitions")?
        .iter()
        .map(|(name, spec)| Ok((name.clone(), definition(spec, events.clone(), gates.clone())?)))
        .collect::<Result<HashMap<_, _>>>()?;
    let mut bindings = HashMap::<String, (String, Context, BindingLease)>::new();
    let mut contexts = HashMap::from([("root".to_owned(), root.clone())]);
    let mut realms = HashMap::new();
    let mut fibers = HashMap::<String, Fiber>::new();
    let mut identities = HashMap::new();
    let mut retiring = Vec::new();
    let mut disposing = Vec::new();
    let mut errors = Vec::new();
    let mut event_scenarios = event_scenarios::EventScenarios::default();
    for action in scenario["actions"].as_array().context("missing actions")? {
        if event_scenarios.execute(action, &root, &events, &mut errors).await? {
            continue;
        }
        let field = |key: &str| action[key].as_str().map(str::to_owned).with_context(|| format!("missing {key}"));
        let context = match action["context"].as_str() {
            Some(name) => contexts.get(name).context("unknown context")?.clone(),
            None => root.clone(),
        };
        match field("op")?.as_str() {
            "tagged" => {
                context.provide(ServiceKey::new(field("service")?), metadata::MetadataService { dependency: None })?;
            }
            "intercept" => {
                let target = field("target")?;
                ensure!(!contexts.contains_key(&target), "duplicate context");
                let parent = contexts.get(&field("parent")?).context("unknown parent context")?;
                let tags: Vec<String> = serde_json::from_value(action["tags"].clone())?;
                let context = parent.intercept(ServiceKey::<metadata::MetadataService>::new(field("service")?), tags);
                contexts.insert(target, context);
            }
            "drain-bindings" => {
                for retired in retiring.drain(..) {
                    retired.await?;
                }
            }
            "counter" => {
                context.provide(
                    ServiceKey::new(field("service")?),
                    Counter { count: Arc::new(AtomicUsize::new(0)), events: events.clone() },
                )?;
            }
            "count" => {
                let name = field("service")?;
                let count = context
                    .service(ServiceKey::<Counter>::new(name.clone()))?
                    .call(|counter, _| counter.count.load(Ordering::SeqCst))?;
                events.lock().unwrap().push(format!("count({name},{count})"));
            }
            "isolate" => {
                let target = field("target")?;
                ensure!(!contexts.contains_key(&target), "duplicate context");
                let realm = *realms.entry(field("realm")?).or_insert_with(RealmId::new);
                let parent = contexts.get(&field("parent")?).context("unknown parent context")?;
                let context = parent.isolate(ServiceKey::<Value>::new(field("service")?), realm);
                contexts.insert(target, context);
            }
            "publish" | "publish-in" => {
                let service = field("service")?;
                let lease = context.provide(ServiceKey::<Value>::new(service.clone()), action["value"].clone())?;
                bindings.insert(field("binding")?, (service, context, lease));
            }
            "mount" | "mount-in" => {
                let component = definitions.get(&field("definition")?).context("unknown definition")?.clone();
                let fiber = context.mount(component, action["config"].clone())?;
                identities.insert(field("fiber")?, fiber.id());
                fibers.insert(field("fiber")?, fiber);
            }
            "replace-component" => {
                let old = definitions.get(&field("definition")?).context("unknown definition")?;
                let next = definitions.get(&field("replacement")?).context("unknown definition")?;
                root.replace_component(old.id(), next.clone())?;
            }
            "identity" => {
                let id = field("fiber")?;
                let same = Some(&fibers.get(&id).context("unknown fiber")?.id()) == identities.get(&id);
                events.lock().unwrap().push(format!("identity({id},{same})"));
            }
            "replace-binding" => {
                let (service, context, lease) = bindings.remove(&field("binding")?).context("unknown binding")?;
                retiring.push(lease.withdraw());
                let lease = context.provide(ServiceKey::<Value>::new(service.clone()), action["value"].clone())?;
                bindings.insert(field("replacement")?, (service, context, lease));
            }
            "withdraw" => {
                let (_, _, lease) = bindings.remove(&field("binding")?).context("unknown binding")?;
                retiring.push(lease.withdraw());
            }
            "dispose" => {
                let fiber = fibers.get(&field("fiber")?).context("unknown fiber")?.clone();
                disposing.push(tokio::spawn(async move { fiber.dispose().await }));
            }
            "wait-gate" => {
                gates
                    .get(&field("gate")?)
                    .context("unknown gate")?
                    .entered
                    .subscribe()
                    .wait_for(|entered| *entered)
                    .await?;
            }
            "open-gate" => {
                gates.get(&field("gate")?).context("unknown gate")?.opened.send_replace(true);
            }
            "update" => {
                fibers.get(&field("fiber")?).context("unknown fiber")?.update(action["config"].clone())?;
            }
            "state" => {
                let id = field("fiber")?;
                let state = state_name(fibers.get(&id).context("unknown fiber")?.state());
                events.lock().unwrap().push(format!("state({id},{state})"));
            }
            "instances" => {
                let id = field("definition")?;
                let component = definitions.get(&id).context("unknown definition")?;
                events.lock().unwrap().push(format!("instances({id},{})", root.instances(component.id()).len()));
            }
            "settle" => {
                if let Err(error) = fibers.get(&field("fiber")?).context("unknown fiber")?.settled().await {
                    errors.push(error.to_string());
                }
                for retired in retiring.drain(..) {
                    retired.await?;
                }
                for disposed in disposing.drain(..) {
                    disposed.await??;
                }
            }
            op => bail!("unknown action: {op}"),
        }
    }
    let result = Outcome {
        events: events.lock().unwrap().clone(),
        states: fibers.iter().map(|(id, fiber)| (id.clone(), state_name(fiber.state()).into())).collect(),
        errors,
    };
    root.shutdown().await;
    ensure!(serde_json::json!(result.events) == scenario["expected"], "trace mismatch: {:?}", result.events);
    ensure!(
        serde_json::json!(result.errors)
            == scenario.get("expected_errors").cloned().unwrap_or_else(|| serde_json::json!([])),
        "error outcome mismatch: {:?}",
        result.errors
    );
    for pair in scenario["before"].as_array().context("missing ordering")? {
        let a = result
            .events
            .iter()
            .position(|event| Some(event.as_str()) == pair[0].as_str())
            .context("missing ordered event")?;
        let b = result
            .events
            .iter()
            .position(|event| Some(event.as_str()) == pair[1].as_str())
            .context("missing ordered event")?;
        ensure!(a < b, "ordering mismatch");
    }
    Ok(result)
}
