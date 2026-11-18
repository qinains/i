use anyhow::{Context as _, Result, bail, ensure};
use i::module::kernel::{Context, Event, EventValue, ListenOptions, Subscription};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct EventScenarios {
    events: HashMap<String, Event<()>>,
    subscriptions: Arc<Mutex<HashMap<String, Subscription>>>,
}

impl EventScenarios {
    pub async fn execute(
        &mut self,
        action: &Value,
        context: &Context,
        trace: &Arc<Mutex<Vec<String>>>,
        errors: &mut Vec<String>,
    ) -> Result<bool> {
        let op = action["op"].as_str().context("missing op")?;
        if !matches!(op, "listen" | "dispatch") {
            return Ok(false);
        }
        let name = action["event"].as_str().context("missing event")?;
        let event = self.events.entry(name.to_owned()).or_insert_with(|| Event::new(name)).clone();
        if op == "listen" {
            let listener = action["listener"].as_str().context("missing listener")?.to_owned();
            ensure!(!self.subscriptions.lock().unwrap().contains_key(&listener), "duplicate listener");
            let behavior = action["behavior"].as_str().context("missing behavior")?.to_owned();
            let events = trace.clone();
            let id = listener.clone();
            let subscription = if matches!(behavior.as_str(), "around" | "twice") {
                event.around(&context.effects(), move |(), next| {
                    let events = events.clone();
                    let listener = listener.clone();
                    let twice = behavior == "twice";
                    async move {
                        events.lock().unwrap().push(format!("listener({listener})"));
                        let value = next.run().await?;
                        if twice {
                            next.run().await?;
                        }
                        events.lock().unwrap().push(format!("after({listener})"));
                        Ok(value)
                    }
                })?
            } else {
                let subscriptions = Arc::downgrade(&self.subscriptions);
                event.on(
                    &context.effects(),
                    move |()| {
                        events.lock().unwrap().push(format!("listener({listener})"));
                        Ok(match behavior.as_str() {
                            "false" => EventValue::Value(json!(false)),
                            "null" => EventValue::Value(Value::Null),
                            "undefined" => EventValue::Undefined,
                            "zero" => EventValue::Value(json!(0)),
                            "empty" => EventValue::Value(json!("")),
                            "short" => EventValue::Value(json!("short")),
                            "fail" => bail!("failed({listener})"),
                            value => {
                                let target = value.strip_prefix("remove:").context("unknown listener behavior")?;
                                subscriptions
                                    .upgrade()
                                    .context("listener scope has finished")?
                                    .lock()
                                    .unwrap()
                                    .get(target)
                                    .context("unknown listener removal")?
                                    .remove();
                                EventValue::Undefined
                            }
                        })
                    },
                    ListenOptions::default(),
                )?
            };
            self.subscriptions.lock().unwrap().insert(id, subscription);
            return Ok(true);
        }
        let mode = action["mode"].as_str().context("missing mode")?;
        let result = match mode {
            "emit" => event.emit(()).map(|()| EventValue::Undefined).map_err(|e| vec![e.to_string()]),
            "bail" => event.bail(()).map_err(|e| vec![e.to_string()]),
            "serial" => event.serial(()).await.map_err(|e| vec![e.to_string()]),
            "parallel" => event.parallel(()).await.map(|()| EventValue::Undefined).map_err(|e| e.errors),
            "waterfall" => {
                let trace = trace.clone();
                let name = name.to_owned();
                event
                    .waterfall((), move || {
                        let trace = trace.clone();
                        let name = name.clone();
                        async move {
                            trace.lock().unwrap().push(format!("tail({name})"));
                            Ok(EventValue::Value(json!("tail")))
                        }
                    })
                    .await
                    .map_err(|e| vec![e.to_string()])
            }
            _ => bail!("unknown event mode"),
        };
        match result {
            Ok(value) => {
                let value = match value {
                    EventValue::Undefined => "undefined".to_owned(),
                    EventValue::Value(value) => value.to_string(),
                };
                trace.lock().unwrap().push(format!("result({name},{mode},{value})"));
            }
            Err(failures) => {
                trace.lock().unwrap().push(format!("error({name},{mode})"));
                errors.extend(failures);
            }
        }
        Ok(true)
    }
}
