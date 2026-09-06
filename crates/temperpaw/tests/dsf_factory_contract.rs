//! Execute the DSF factory IOA contract through Temper's production evaluator.
use std::{fs, path::PathBuf, sync::Arc};

use serde_json::{Value, json};
use temper_jit::table::TransitionTable;
use temper_runtime::scheduler::{FaultConfig, SimActorSystem, SimActorSystemConfig};
use temper_server::entity_actor::sim_handler::EntityActorHandler;

const ENTITIES: &[(&str, &str)] = &[
    ("resource", "DsfResource"),
    ("flow", "DsfFlow"),
    ("participant", "DsfParticipant"),
    ("observation", "DsfObservation"),
    ("operation", "DsfOperation"),
    ("model_sync", "DsfModelSync"),
    ("experiment", "DsfExperiment"),
];

fn source(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../os-apps/dsf-factory/specs/{name}.ioa.toml"));
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn simulator(name: &str, entity: &str, seed: u64) -> SimActorSystem {
    let source = source(name);
    let table = Arc::new(TransitionTable::from_ioa_source(&source));
    let id = if entity == "DsfOperation" {
        "operation-1"
    } else {
        "subject"
    };
    let handler = EntityActorHandler::new(entity, id, table).with_ioa_invariants(&source);
    let mut sim = SimActorSystem::new(SimActorSystemConfig {
        seed,
        faults: FaultConfig::none(),
        ..Default::default()
    });
    sim.register_actor("subject", Box::new(handler));
    sim
}

fn step(sim: &mut SimActorSystem, action: &str, params: Value) -> Value {
    sim.step("subject", action, &params.to_string())
        .unwrap_or_else(|error| panic!("{action}: {error}"))
}

#[test]
fn every_factory_spec_runs_in_the_production_simulator() {
    for (name, entity) in ENTITIES {
        let sim = simulator(name, entity, 1);
        sim.assert_status("subject", "Draft");
        assert!(!sim.has_violations());
    }
}

#[test]
fn observation_cannot_overwrite_intended_configuration() {
    let mut sim = simulator("resource", "DsfResource", 2);
    step(
        &mut sim,
        "Register",
        json!({
            "kind": "api", "provider": "railway", "provider_id": "api-production",
            "intended_configuration": "approved-config", "source_repository": "arni-labs/deep-sci-fi"
        }),
    );
    let result = sim.step(
        "subject",
        "Observe",
        &json!({
            "observation_id": "sample-1", "observed_configuration": "drifted-config",
        "observed_revision": "revision-1", "expected_sequence": 0, "observed_at_ms": 1000,
        "coverage": "measured", "provenance_ref": "provider-query-1",
            "intended_configuration": "attacker-config"
        })
        .to_string(),
    );
    assert!(
        result.is_err(),
        "Observe must reject an undeclared intended_configuration parameter"
    );
    let state = step(
        &mut sim,
        "Observe",
        json!({
            "observation_id": "sample-1", "observed_configuration": "drifted-config",
            "observed_revision": "revision-1", "expected_sequence": 0, "observed_at_ms": 1000,
            "coverage": "measured", "provenance_ref": "provider-query-1"
        }),
    );
    assert_eq!(state["fields"]["intended_configuration"], "approved-config");
    assert_eq!(state["fields"]["observed_configuration"], "drifted-config");
}

#[test]
fn accepted_operation_target_cannot_change_on_execution() {
    let mut sim = simulator("operation", "DsfOperation", 3);
    step(
        &mut sim,
        "Request",
        json!({
            "resource_id": "api-production", "effort_id": "effort-1", "operation_kind": "deploy",
            "operation_key": "operation-1", "intended_revision": "revision-1"
        }),
    );
    let result = sim.step("subject", "Validate", r#"{"resource_id":"other-resource"}"#);
    assert!(
        result.is_err(),
        "Validate must not accept a replacement target"
    );
}

fn registered_resource(seed: u64) -> SimActorSystem {
    let mut sim = simulator("resource", "DsfResource", seed);
    step(
        &mut sim,
        "Register",
        json!({
            "kind": "api", "provider": "railway", "provider_id": "api-production",
            "intended_configuration": "approved-config", "source_repository": "arni-labs/deep-sci-fi"
        }),
    );
    sim
}

fn observation(sequence: u64, observed_at_ms: u64) -> Value {
    json!({"observation_id": format!("observation-{observed_at_ms}"),
        "observed_configuration": "provider-config", "observed_revision": "revision-1",
        "coverage": "measured", "outcome": "drift", "provenance_ref": "provider-query-1",
        "observed_at_ms": observed_at_ms, "expected_sequence": sequence})
}

#[test]
fn replay_and_out_of_order_observations_never_replace_newer_facts() {
    for seed in 1..=64 {
        let mut sim = registered_resource(seed);
        let first = observation(0, 1000 + seed);
        let state = step(&mut sim, "Observe", first.clone());
        assert_eq!(state["fields"]["observed_sequence"], 1, "seed={seed}");
        assert!(
            sim.step("subject", "Observe", &first.to_string()).is_err(),
            "replay seed={seed}"
        );
        let stale = observation(1, 999 + seed);
        assert!(
            sim.step("subject", "Observe", &stale.to_string()).is_err(),
            "out of order seed={seed}"
        );
        let concurrent = observation(0, 2000 + seed);
        assert!(
            sim.step("subject", "Observe", &concurrent.to_string())
                .is_err(),
            "lost CAS seed={seed}"
        );
        let state = step(&mut sim, "Observe", observation(1, 2000 + seed));
        assert_eq!(state["fields"]["observed_sequence"], 2, "seed={seed}");
        assert_eq!(
            state["fields"]["intended_configuration"], "approved-config",
            "seed={seed}"
        );
        sim.assert_event_count("subject", 3);
        assert!(!sim.has_violations(), "seed={seed}: {:?}", sim.violations());
    }
}

fn requested_operation(seed: u64) -> SimActorSystem {
    let mut sim = simulator("operation", "DsfOperation", seed);
    step(
        &mut sim,
        "Request",
        json!({
            "resource_id": "api-production", "effort_id": "effort-1", "operation_kind": "deploy",
            "operation_key": "operation-1", "intended_revision": "revision-1", "proof_ref": "proof-1"
        }),
    );
    sim
}

fn ready_operation(seed: u64) -> SimActorSystem {
    let mut sim = requested_operation(seed);
    step(&mut sim, "Validate", json!({}));
    step(
        &mut sim,
        "ValidationSucceeded",
        json!({
            "operation_key": "operation-1", "validation_evidence_ref": "validation-1"
        }),
    );
    sim
}

fn provider_result() -> Value {
    json!({"operation_key":"operation-1", "provider_execution_id":"deployment-1",
        "provider_evidence_ref":"provider-query-1"})
}

fn verified_evidence() -> Value {
    json!({"operation_key":"operation-1", "verified_resource_id":"api-production",
        "verified_revision":"revision-1", "provider_evidence_ref":"provider-query-1",
        "flow_evidence_ref":"browser-proof-1", "telemetry_evidence_ref":"datadog-proof-1"})
}

#[test]
fn all_accepted_target_fields_reject_mutation_before_any_external_write() {
    for field in [
        "resource_id",
        "effort_id",
        "operation_kind",
        "operation_key",
        "intended_revision",
    ] {
        let mut sim = ready_operation(10);
        let result = sim.step(
            "subject",
            "Execute",
            &json!({field:"replacement"}).to_string(),
        );
        assert!(result.is_err(), "Execute accepted immutable field {field}");
        sim.assert_status("subject", "Ready");
        sim.assert_event_count("subject", 3);
        let state = step(&mut sim, "Execute", json!({}));
        assert_eq!(state["fields"]["execution_attempts"], 1);
        assert_eq!(state["fields"]["operation_key"], "operation-1");
    }
}

#[test]
fn unknown_provider_outcome_requires_observation_before_retry() {
    let mut sim = ready_operation(11);
    step(&mut sim, "Execute", json!({}));
    step(
        &mut sim,
        "ExecutionUncertain",
        json!({"error_message":"response lost"}),
    );
    assert!(sim.step("subject", "Execute", "{}").is_err());
    step(&mut sim, "Reconcile", json!({}));
    step(&mut sim, "ProviderFound", provider_result());
    assert!(sim.step("subject", "Execute", "{}").is_err());
    assert!(
        sim.step(
            "subject",
            "ProviderAbsent",
            &json!({
                "operation_key":"operation-1", "absence_evidence_ref":"late-absence"
            })
            .to_string()
        )
        .is_err()
    );
    let state = step(&mut sim, "Verify", json!({}));
    assert_eq!(state["fields"]["execution_attempts"], 1);
    assert_eq!(state["fields"]["provider_execution_id"], "deployment-1");
}

#[test]
fn retries_are_bounded_and_keep_the_original_operation_key() {
    let mut sim = ready_operation(12);
    for attempt in 1..=3 {
        let state = step(&mut sim, "Execute", json!({}));
        assert_eq!(state["fields"]["execution_attempts"], attempt);
        assert_eq!(state["fields"]["operation_key"], "operation-1");
        step(
            &mut sim,
            "ExecutionUncertain",
            json!({"error_message":"response lost"}),
        );
        step(&mut sim, "Reconcile", json!({}));
        let result = sim.step(
            "subject",
            "ProviderAbsent",
            &json!({
                "operation_key":"operation-1", "absence_evidence_ref":"provider-lookup"
            })
            .to_string(),
        );
        assert_eq!(result.is_ok(), attempt < 3);
    }
    assert!(!sim.has_violations(), "{:?}", sim.violations());
}

#[test]
fn verification_requires_matching_target_and_available_flow_telemetry_evidence() {
    for (field, wrong) in [
        ("verified_resource_id", "another-resource"),
        ("verified_revision", "wrong-revision"),
        ("operation_key", "another-operation"),
        ("provider_evidence_ref", ""),
        ("flow_evidence_ref", ""),
        ("telemetry_evidence_ref", ""),
    ] {
        let mut sim = ready_operation(13);
        step(&mut sim, "Execute", json!({}));
        step(&mut sim, "ExecutionSucceeded", provider_result());
        step(&mut sim, "Verify", json!({}));
        let mut evidence = verified_evidence();
        evidence[field] = json!(wrong);
        assert!(
            sim.step("subject", "VerificationSucceeded", &evidence.to_string())
                .is_err(),
            "accepted {field}={wrong:?}"
        );
        sim.assert_status("subject", "Verifying");
        let state = step(&mut sim, "VerificationSucceeded", verified_evidence());
        assert_eq!(state["fields"]["telemetry_verified"], true);
        sim.assert_status("subject", "Verified");
        assert!(sim.step("subject", "Execute", "{}").is_err());
        assert!(!sim.has_violations(), "{:?}", sim.violations());
    }
}

fn configured_experiment(database: &str, bucket: &str) -> SimActorSystem {
    let mut sim = simulator("experiment", "DsfExperiment", 14);
    step(
        &mut sim,
        "Configure",
        json!({
            "effort_id":"experiment-effort", "branch":"codex/variant-one", "source_revision":"revision-1",
            "computer_id":"experiment-computer", "database_id":database, "media_bucket":bucket,
            "media_namespace":"variant-one", "permitted_external_calls":"[]"
        }),
    );
    step(&mut sim, "Validate", json!({}));
    sim
}

fn isolation_evidence() -> Value {
    json!({"production_database_id":"production-db", "production_media_bucket":"production-media",
        "isolation_evidence_ref":"binding-check-1"})
}

#[test]
fn experiment_cannot_use_production_database_or_media_bucket() {
    for (database, bucket) in [
        ("production-db", "experiment-media"),
        ("experiment-db", "production-media"),
    ] {
        let mut sim = configured_experiment(database, bucket);
        assert!(
            sim.step(
                "subject",
                "IsolationSucceeded",
                &isolation_evidence().to_string()
            )
            .is_err()
        );
        assert!(sim.step("subject", "Run", "{}").is_err());
        sim.assert_status("subject", "Validating");
    }
    let mut sim = configured_experiment("experiment-db", "experiment-media");
    step(&mut sim, "IsolationSucceeded", isolation_evidence());
    assert!(
        sim.step("subject", "Run", r#"{"database_id":"production-db"}"#)
            .is_err()
    );
    step(&mut sim, "Run", json!({}));
    step(
        &mut sim,
        "RunSucceeded",
        json!({"result_ref":"variant-1", "test_evidence_ref":"tests-1"}),
    );
    step(
        &mut sim,
        "Select",
        json!({"selection_ask_id":"ask-1", "delivery_effort_id":"delivery-1"}),
    );
    assert!(sim.step("subject", "Deploy", "{}").is_err());
    step(&mut sim, "Cleanup", json!({}));
    step(
        &mut sim,
        "CleanupSucceeded",
        json!({"cleanup_evidence_ref":"cleanup-1"}),
    );
    sim.assert_status("subject", "Cleaned");
    assert!(!sim.has_violations());
}

#[test]
fn recorded_observations_are_immutable_for_every_coverage_outcome() {
    for coverage in ["Measured", "Absent", "Inaccessible", "Stale"] {
        let mut sim = simulator("observation", "DsfObservation", 15);
        let params = json!({"subject_type":"DsfResource", "subject_id":"api-production",
            "source":"datadog", "source_event_id":"query-1", "query":"service:dsf",
            "window_start":"2026-09-06T00:00:00Z", "window_end":"2026-09-06T01:00:00Z",
            "observed_at_ms":1000, "sample_kind":"sampled-spans", "outcome":"unknown",
            "summary":"query evidence", "evidence_ref":"query-1"});
        step(&mut sim, &format!("Record{coverage}"), params.clone());
        for replacement in ["Measured", "Absent", "Inaccessible", "Stale"] {
            assert!(
                sim.step(
                    "subject",
                    &format!("Record{replacement}"),
                    &params.to_string()
                )
                .is_err()
            );
        }
        sim.assert_status("subject", coverage);
        sim.assert_event_count("subject", 1);
        assert!(!sim.has_violations());
    }
}

#[test]
fn known_provider_execution_stays_nonrepeatable_under_scheduler_faults() {
    for seed in 1..=64 {
        let source = source("operation");
        let table = Arc::new(TransitionTable::from_ioa_source(&source));
        let handler = EntityActorHandler::new("DsfOperation", "operation-1", table)
            .with_ioa_invariants(&source);
        let mut sim = SimActorSystem::new(SimActorSystemConfig {
            seed,
            max_ticks: 150,
            max_actions_per_actor: 40,
            faults: FaultConfig::heavy(),
        });
        sim.register_actor("subject", Box::new(handler));
        step(
            &mut sim,
            "Request",
            json!({"resource_id":"api-production", "effort_id":"effort-1",
            "operation_kind":"deploy", "operation_key":"operation-1", "intended_revision":"revision-1"}),
        );
        step(&mut sim, "Validate", json!({}));
        step(
            &mut sim,
            "ValidationSucceeded",
            json!({"operation_key":"operation-1", "validation_evidence_ref":"validation-1"}),
        );
        step(&mut sim, "Execute", json!({}));
        step(&mut sim, "ExecutionSucceeded", provider_result());
        let result = sim.run_random();
        assert!(
            result.all_invariants_held,
            "seed={seed}: {:?}",
            sim.violations()
        );
        let events = sim.events_json("subject");
        let writes = events
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["action"] == "Execute")
            .count();
        assert_eq!(writes, 1, "duplicate write seed={seed}: {events}");
    }
}

#[test]
fn inaccessible_observation_preserves_the_last_measured_configuration() {
    let mut sim = registered_resource(17);
    step(&mut sim, "Observe", observation(0, 1000));
    let unavailable = json!({"observation_id":"denied-1", "coverage":"inaccessible",
        "provenance_ref":"provider-403", "observed_at_ms":2000, "expected_sequence":1});
    let state = step(&mut sim, "ObserveUnavailable", unavailable);
    assert_eq!(state["fields"]["observed_configuration"], "provider-config");
    assert_eq!(state["fields"]["intended_configuration"], "approved-config");
    assert_eq!(state["fields"]["observation_available"], false);
}

#[test]
fn failed_sync_preserves_last_success_and_pause_rejects_late_callbacks() {
    let mut sim = simulator("model_sync", "DsfModelSync", 18);
    step(
        &mut sim,
        "Configure",
        json!({"source_kind":"datadog", "source_id":"backend", "resource_id":"api-production",
        "source_config_ref":"datadog-query-config"}),
    );
    assert!(sim.step("subject", "CollectionSucceeded", "{}").is_err());
    step(&mut sim, "Refresh", json!({}));
    let success = collection_evidence();
    step(&mut sim, "CollectionSucceeded", success.clone());
    step(&mut sim, "Refresh", json!({}));
    assert!(
        sim.step("subject", "CollectionSucceeded", &success.to_string())
            .is_err()
    );
    let state = step(
        &mut sim,
        "CollectionFailed",
        json!({"error_message":"provider denied access"}),
    );
    assert_eq!(state["fields"]["last_success_at"], "2026-09-06T01:00:00Z");
    assert_eq!(state["fields"]["evidence_ref"], "provider-query-1");
    assert_eq!(state["fields"]["failure_count"], 1);
    step(&mut sim, "Pause", json!({}));
    sim.assert_status("subject", "Paused");
    assert!(
        sim.step("subject", "CollectionSucceeded", &success.to_string())
            .is_err()
    );
}

fn pascal_case(name: &str) -> String {
    name.split('_')
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect()
}

#[test]
fn csdl_matches_every_declared_ioa_field_action_and_parameter() {
    use std::collections::BTreeMap;
    let csdl_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../os-apps/dsf-factory/specs/model.csdl.xml");
    let document = temper_spec::csdl::parse_csdl(&fs::read_to_string(csdl_path).unwrap()).unwrap();
    let schema = &document.schemas[0];
    assert_eq!(schema.entity_types.len(), ENTITIES.len());
    for (file, name) in ENTITIES {
        let ioa = temper_spec::automaton::parse_automaton(&source(file)).unwrap();
        let entity = schema
            .entity_types
            .iter()
            .find(|entity| entity.name == *name)
            .unwrap();
        let field_types: BTreeMap<_, _> = entity
            .properties
            .iter()
            .filter(|field| !["Id", "Status"].contains(&field.name.as_str()))
            .map(|field| (field.name.clone(), field.type_name.clone()))
            .collect();
        let expected_types: BTreeMap<_, _> = ioa
            .state
            .iter()
            .map(|field| {
                let kind = match field.var_type.as_str() {
                    "counter" => "Edm.Int64",
                    "bool" => "Edm.Boolean",
                    "string" => "Edm.String",
                    other => panic!("unexpected field type {other}"),
                };
                (pascal_case(&field.name), kind.to_owned())
            })
            .collect();
        assert_eq!(field_types, expected_types, "{name} properties");
        let binding = format!("Dsf.Factory.{name}");
        let actions: BTreeMap<_, _> = schema
            .actions
            .iter()
            .filter(|action| {
                action
                    .parameters
                    .first()
                    .is_some_and(|parameter| parameter.type_name == binding)
            })
            .map(|action| (action.name.as_str(), action))
            .collect();
        assert_eq!(actions.len(), ioa.actions.len(), "{name} action count");
        for action in &ioa.actions {
            let served = actions[action.name.as_str()];
            let expected: Vec<_> = action.params.iter().map(|param| param.name()).collect();
            let actual: Vec<_> = served
                .parameters
                .iter()
                .skip(1)
                .map(|param| param.name.as_str())
                .collect();
            assert_eq!(actual, expected, "{name}.{} parameters", action.name);
        }
    }
}

fn operation_request() -> Value {
    json!({"active_operation_id":"operation-1", "request_effort_id":"effort-1",
        "request_operation_kind":"deploy", "request_revision":"revision-1",
        "request_configuration":"approved-config", "request_proof_ref":"proof-1"})
}

#[test]
fn resource_serializes_operations_and_rejects_unrelated_release() {
    let mut sim = registered_resource(19);
    step(&mut sim, "RequestOperation", operation_request());
    sim.assert_status("subject", "Operating");
    let mut another = operation_request();
    another["active_operation_id"] = json!("operation-2");
    assert!(
        sim.step("subject", "RequestOperation", &another.to_string())
            .is_err()
    );
    assert!(
        sim.step(
            "subject",
            "ReleaseOperation",
            r#"{"operation_key":"operation-2"}"#
        )
        .is_err()
    );
    step(&mut sim, "Observe", observation(0, 1000));
    sim.assert_status("subject", "Operating");
    step(
        &mut sim,
        "ReleaseOperation",
        json!({"operation_key":"operation-1"}),
    );
    sim.assert_status("subject", "Active");
    step(&mut sim, "RequestOperation", another);
    assert!(
        sim.step(
            "subject",
            "ReleaseOperation",
            r#"{"operation_key":"operation-1"}"#
        )
        .is_err()
    );
}

fn collection_evidence() -> Value {
    json!({"expected_sequence":1, "observation_id":"observation-1", "source_event_id":"source-event-1",
        "query":"service:deep-sci-fi-backend", "window_start":"2026-09-06T00:00:00Z",
        "window_end":"2026-09-06T01:00:00Z", "sample_kind":"sampled-spans", "outcome":"healthy",
        "summary":"request samples inspected", "evidence_ref":"provider-query-1", "observed_at_ms":2000,
        "expected_resource_sequence":0, "source_cursor":"cursor-1", "last_success_at":"2026-09-06T01:00:00Z",
        "next_due_at":"2030-01-01T00:00:00Z", "observed_configuration":"provider-config", "observed_revision":"revision-1"})
}

fn reaction_simulator() -> temper_server::trigger::sim_dispatcher::SimReactionSystem {
    use temper_server::{registry::SpecRegistry, trigger::sim_dispatcher::SimReactionSystem};
    let xml = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../os-apps/dsf-factory/specs/model.csdl.xml"),
    )
    .unwrap();
    let csdl = temper_spec::csdl::parse_csdl(&xml).unwrap();
    let sources: Vec<_> = ENTITIES
        .iter()
        .map(|(file, entity)| (*entity, source(file)))
        .collect();
    let borrowed: Vec<_> = sources
        .iter()
        .map(|(entity, ioa)| (*entity, ioa.as_str()))
        .collect();
    let mut registry = SpecRegistry::new();
    registry.register_tenant("dsf-test", csdl, xml, &borrowed);
    let mut sim = SimReactionSystem::new(
        SimActorSystemConfig {
            seed: 23,
            faults: FaultConfig::none(),
            ..Default::default()
        },
        registry.build_reaction_registry(),
        "dsf-test",
    );
    for (file, entity, actor, id) in [
        ("resource", "DsfResource", "resource", "api-production"),
        ("operation", "DsfOperation", "operation", "operation-1"),
        ("model_sync", "DsfModelSync", "sync", "sync-1"),
        (
            "observation",
            "DsfObservation",
            "observation",
            "observation-1",
        ),
    ] {
        let ioa = source(file);
        let table = Arc::new(TransitionTable::from_ioa_source(&ioa));
        let handler = EntityActorHandler::new(entity, id, table).with_ioa_invariants(&ioa);
        sim.register_actor(actor, entity, id, Box::new(handler));
    }
    sim.step(
        "resource",
        "Register",
        &json!({"kind":"api", "provider":"railway",
        "provider_id":"api-production", "intended_configuration":"approved-config"})
        .to_string(),
    )
    .unwrap();
    sim
}

#[test]
fn declared_resource_operation_chain_releases_only_its_terminal_child() {
    let mut sim = reaction_simulator();
    sim.step(
        "resource",
        "RequestOperation",
        &operation_request().to_string(),
    )
    .unwrap();
    sim.assert_status("resource", "Operating");
    sim.assert_status("operation", "Validating");
    assert!(
        sim.last_results().iter().all(|result| result.success),
        "{:?}",
        sim.last_results()
    );
    sim.step(
        "operation",
        "ValidationSucceeded",
        &json!({"operation_key":"operation-1",
        "validation_evidence_ref":"validation-1"})
        .to_string(),
    )
    .unwrap();
    sim.assert_status("operation", "Executing");
    sim.step(
        "operation",
        "ExecutionUncertain",
        r#"{"error_message":"response lost"}"#,
    )
    .unwrap();
    sim.assert_status("resource", "Operating");
    sim.step("operation", "Reconcile", "{}").unwrap();
    sim.step("operation", "ProviderFound", &provider_result().to_string())
        .unwrap();
    sim.assert_status("operation", "Verifying");
    sim.assert_status("resource", "Operating");
    sim.step(
        "operation",
        "VerificationSucceeded",
        &verified_evidence().to_string(),
    )
    .unwrap();
    sim.assert_status("operation", "Verified");
    sim.assert_status("resource", "Active");
    assert!(
        sim.last_results().iter().all(|result| result.success),
        "{:?}",
        sim.last_results()
    );
    assert!(!sim.has_violations());
}

fn start_collection(sim: &mut temper_server::trigger::sim_dispatcher::SimReactionSystem) {
    sim.step(
        "sync",
        "Configure",
        &json!({"source_kind":"datadog", "source_id":"backend",
        "resource_id":"api-production", "source_config_ref":"query-config"})
        .to_string(),
    )
    .unwrap();
    sim.step("sync", "Refresh", "{}").unwrap();
    sim.assert_status("sync", "Collecting");
}

#[test]
fn collection_records_evidence_before_projecting_to_the_resource() {
    let mut sim = reaction_simulator();
    start_collection(&mut sim);
    sim.step(
        "sync",
        "CollectionSucceeded",
        &collection_evidence().to_string(),
    )
    .unwrap();
    sim.assert_status("observation", "Measured");
    sim.assert_status("sync", "Ready");
    assert_eq!(sim.last_results().len(), 2);
    assert!(
        sim.last_results().iter().all(|result| result.success),
        "{:?}",
        sim.last_results()
    );
    let state = sim
        .step("resource", "Observe", &observation(1, 3000).to_string())
        .unwrap();
    assert_eq!(state["fields"]["observed_sequence"], 2);
    assert_eq!(state["fields"]["intended_configuration"], "approved-config");
}

#[test]
fn stale_projection_keeps_the_recorded_evidence_and_newer_resource_facts() {
    let mut sim = reaction_simulator();
    sim.step("resource", "Observe", &observation(0, 2500).to_string())
        .unwrap();
    start_collection(&mut sim);
    sim.step(
        "sync",
        "CollectionSucceeded",
        &collection_evidence().to_string(),
    )
    .unwrap();
    sim.assert_status("observation", "Measured");
    let reactions = sim.last_results();
    assert_eq!(reactions.len(), 2);
    assert!(
        reactions[0].success,
        "immutable observation must be committed first"
    );
    assert!(!reactions[1].success, "stale resource projection must fail");
    let state = sim
        .step("resource", "Observe", &observation(1, 3000).to_string())
        .unwrap();
    assert_eq!(state["fields"]["observed_sequence"], 2);
}

#[test]
fn counter_assignments_only_read_declared_action_parameters() {
    for (file, name) in ENTITIES {
        let ioa = temper_spec::automaton::parse_automaton(&source(file)).unwrap();
        for action in ioa.actions {
            for effect in action.effect {
                if let temper_spec::automaton::Effect::SetCounterFromParam { param, .. } = effect {
                    assert!(
                        action
                            .params
                            .iter()
                            .any(|declared| declared.name() == param),
                        "{name}.{} reads missing counter parameter {param}",
                        action.name
                    );
                }
            }
        }
    }
}

#[test]
fn successful_sync_resets_consecutive_failure_budget() {
    let mut sim = simulator("model_sync", "DsfModelSync", 26);
    step(
        &mut sim,
        "Configure",
        json!({"source_kind":"datadog", "source_id":"backend",
        "resource_id":"telemetry-backend", "source_config_ref":"query-config"}),
    );
    for sequence in 1..=2 {
        step(&mut sim, "Refresh", json!({}));
        let state = step(
            &mut sim,
            "CollectionFailed",
            json!({"error_message":"transient read failure"}),
        );
        assert_eq!(state["fields"]["failure_count"], sequence);
    }
    step(&mut sim, "Refresh", json!({}));
    let mut result = collection_evidence();
    result["expected_sequence"] = json!(3);
    let state = step(&mut sim, "CollectionSucceeded", result);
    assert_eq!(state["fields"]["failure_count"], 0);
}

#[test]
fn numeric_timestamp_strings_cannot_commit_without_advancing_the_timestamp() {
    let mut sim = registered_resource(27);
    let mut candidate = observation(0, 1000);
    candidate["observed_at_ms"] = json!("1000");
    match sim.step("subject", "Observe", &candidate.to_string()) {
        Ok(state) => assert_eq!(
            state["fields"]["observed_at_ms"], 1000,
            "accepted timestamp must be stored by the same numeric interpretation used in its guard"
        ),
        Err(_) => sim.assert_event_count("subject", 1),
    }
}

#[test]
fn operation_key_must_equal_its_canonical_entity_identity() {
    let mut sim = simulator("operation", "DsfOperation", 28);
    let request = json!({"resource_id":"api-production", "effort_id":"effort-1",
        "operation_kind":"deploy", "operation_key":"alias-for-operation-1", "intended_revision":"revision-1"});
    assert!(
        sim.step("subject", "Request", &request.to_string())
            .is_err()
    );
    sim.assert_status("subject", "Draft");
    sim.assert_event_count("subject", 0);
}
