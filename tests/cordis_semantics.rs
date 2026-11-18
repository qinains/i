#![cfg(not(target_arch = "wasm32"))]

#[path = "support/cordis_scenarios.rs"]
mod scenarios;

#[tokio::test]
async fn replacement_teardown_keeps_old_binding() {
    let fixture = serde_json::from_str(include_str!("fixtures/cordis/binding-replacement.json")).unwrap();
    let result = scenarios::run_scenario(&fixture).await.unwrap();
    assert_eq!(result.events, ["load(1)", "unload(1)", "load(2)"]);
}

macro_rules! shared {
    ($test:ident, $file:literal) => {
        #[tokio::test]
        async fn $test() {
            let fixture = serde_json::from_str(include_str!(concat!("fixtures/cordis/", $file, ".json"))).unwrap();
            tokio::time::timeout(std::time::Duration::from_secs(5), scenarios::run_scenario(&fixture))
                .await
                .expect("scenario did not drain its symbolic gates")
                .unwrap();
        }
    };
}

shared!(pending_dependency, "pending-dependency");
shared!(multiple_instances, "multiple-instances");
shared!(unregistered_consumer, "unregistered-consumer");
shared!(withdraw_during_setup, "withdraw-during-setup");
shared!(independent_loading, "independent-loading");
shared!(transitive_drain, "transitive-drain");
shared!(latest_update_during_setup, "latest-update-during-setup");
shared!(component_replacement, "component-replacement");
shared!(failed_refresh, "failed-refresh");
shared!(shared_realms, "shared-realms");
shared!(consumer_owned_effects, "consumer-owned-effects");
shared!(event_modes, "event-modes");
shared!(provider_readiness, "provider-readiness");
shared!(sibling_retirement, "sibling-retirement");
shared!(latest_update_during_unload, "latest-update-during-unload");
shared!(scoped_interception, "scoped-interception");
shared!(service_cycle, "service-cycle");

#[tokio::test]
async fn rejects_duplicate_gate_before_execution() {
    let mut fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/cordis/binding-replacement.json")).unwrap();
    fixture["gates"] = serde_json::json!(["setup", "setup"]);
    let error = scenarios::run_scenario(&fixture).await.err().expect("duplicate gate was accepted");
    assert!(error.to_string().contains("duplicate gate"));
}

#[tokio::test]
async fn rejects_contradictory_ordering_before_execution() {
    let mut fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/cordis/binding-replacement.json")).unwrap();
    fixture["before"] = serde_json::json!([["load(1)", "unload(1)"], ["unload(1)", "load(1)"]]);
    let error = scenarios::run_scenario(&fixture).await.err().expect("ordering cycle was accepted");
    assert!(error.to_string().contains("ordering cycle"));
}
