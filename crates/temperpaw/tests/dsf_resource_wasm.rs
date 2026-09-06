//! Invoke packaged provider stages through the real Temper WASM engine.
//! HTTP fixtures model providers; live provider acceptance is a separate proof.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use temper_wasm::{
    SimWasmHost, StreamRegistry, WasmEngine, WasmInvocationContext, WasmResourceLimits,
};

fn app() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../os-apps/dsf-factory")
}

fn context(entity: &str, module: &str, state: Value) -> WasmInvocationContext {
    WasmInvocationContext {
        tenant: "default".into(),
        entity_type: entity.into(),
        entity_id: "railway-p-s-e".into(),
        trigger_action: "Test".into(),
        wasm_module: Some(module.into()),
        trigger_params: json!({}),
        entity_state: state,
        agent_id: None,
        session_id: None,
        integration_config: BTreeMap::from([
            ("temper_api_url".into(), "https://temper.test".into()),
            ("temper_api_key".into(), "test-only".into()),
        ]),
        trace_id: String::new(),
        workflow_root_entity_type: None,
        workflow_root_entity_id: None,
        workflow_run_id: None,
        http_request: None,
    }
}

fn bytes(module: &str) -> Vec<u8> {
    std::fs::read(app().join(format!("wasm/{module}/{module}.wasm"))).unwrap_or_else(|error| {
        panic!("Build dsf-factory WASMs before this proof: {module}: {error}")
    })
}

fn row(phase: &str, digest: &str) -> Value {
    json!({"status":phase,"operation_sequence":2,"operation_key":"change-2",
        "effort_id":"effort-1","request_revision":"", "request_configuration":"{\"numReplicas\":2}",
        "proof_ref":"proof-1","config_ref":"config-1","config_sha256":digest,"execution_attempts":1,
        "project_id":"project-1","service_id":"service-1","environment_id":"env-1",
        "allowed_operations":["ApplyConfiguration"]})
}

#[tokio::test(flavor = "multi_thread")]
async fn every_packaged_action_stage_emits_its_declared_correlated_failure() {
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(app().join("specs/module-contracts.json")).unwrap())
            .unwrap();
    let engine = WasmEngine::new().unwrap();
    let mut invoked = 0;
    for (entity, resource) in manifest["resources"].as_object().unwrap() {
        for (module, contract) in resource["modules"].as_object().unwrap() {
            if module.ends_with("_collect") {
                continue;
            }
            let (suffix, phase) = if module.ends_with("_validate") {
                ("Validate", "Validating")
            } else if module.ends_with("_execute") {
                ("Execute", "Executing")
            } else if module.ends_with("_observe") {
                ("Reconcile", "Reconciling")
            } else {
                ("Verify", "Verifying")
            };
            let action = contract["action"]
                .as_str()
                .unwrap()
                .strip_suffix(suffix)
                .unwrap();
            let ctx = context(
                entity,
                module,
                row(&format!("{action}{phase}"), &"a".repeat(64)),
            );
            let hash = engine.compile_and_cache(&bytes(module)).unwrap();
            let result = engine
                .invoke(
                    &hash,
                    &ctx,
                    Arc::new(SimWasmHost::new().with_default_response(403, "not accessible")),
                    &WasmResourceLimits::default(),
                    Arc::new(RwLock::new(StreamRegistry::default())),
                )
                .await
                .unwrap();
            assert!(result.success, "{module}: {result:?}");
            let shape = contract["callbacks"]
                .get(&result.callback_action)
                .unwrap_or_else(|| {
                    panic!(
                        "{module} returned undeclared callback {}",
                        result.callback_action
                    )
                });
            let expected = manifest["callback_shapes"][shape.as_str().unwrap()]
                .as_object()
                .unwrap();
            let actual = result.callback_params.as_object().unwrap();
            assert_eq!(
                actual.keys().collect::<Vec<_>>(),
                expected.keys().collect::<Vec<_>>(),
                "{module}"
            );
            assert_eq!(actual["operation_key"], "change-2", "{module}");
            assert_eq!(actual["expected_operation_sequence"], 2, "{module}");
            assert!(!result.callback_params.to_string().contains("test-only"));
            invoked += 1;
        }
    }
    assert_eq!(invoked, 44);
}

#[tokio::test(flavor = "multi_thread")]
async fn packaged_railway_validation_checks_actual_files_and_exact_proof_binding() {
    let config = json!({"version":2,"resource_id":"railway-p-s-e",
        "target":{"project_id":"project-1","service_id":"service-1","environment_id":"env-1","token_secret":"railway-test"},
        "verification":{"flow":{"kind":"provider_configuration"},"datadog":{"site":"datadoghq.com","service":"deep-sci-fi-backend","environment":"production","api_key_secret":"dd-api","app_key_secret":"dd-app"}}});
    let config = config.to_string();
    let digest = format!("{:x}", Sha256::digest(config.as_bytes()));
    let state = row("ApplyConfigurationValidating", &digest);
    let sha = "b".repeat(40);
    let effort = json!({"status":"Merged","head_sha":sha,"proof_packet_id":"proof-1", "ask_ids":[],"proof_attached":true,"e2e_ok":true,"review_passed":true,"evaluation_passed":true});
    let proof = json!({"status":"Recorded","record_present":true,"effort_id":"effort-1","commit":sha,"artifact_ref":"artifact-1",
        "changed_surface":["configuration"],"blast_radius":[],"features":[{"key":"configuration","verification":"rerun","verdict":"pass"}],
        "tests":{"result":"pass"},"independent_verifier":{"agrees":true,"reran":["configuration"]}});
    let mut artifact = json!({"resource_change":{"resource_id":"railway-p-s-e","entity_type":"DsfRailwayServiceInstance","action":"ApplyConfiguration",
        "operation_key":"change-2","operation_sequence":2,"revision":"","configuration_sha256":format!("{:x}", Sha256::digest(state["request_configuration"].as_str().unwrap().as_bytes()))}});
    let engine = WasmEngine::new().unwrap();
    let module = "dsf_railway_configuration_validate";
    let hash = engine.compile_and_cache(&bytes(module)).unwrap();
    for valid in [true, false] {
        if !valid {
            artifact["resource_change"]["operation_sequence"] = 1.into();
        }
        let host = SimWasmHost::new()
            .with_default_response(500, "unexpected request")
            .with_response(
                "https://temper.test/tdata/DsfRailwayServiceInstances('railway-p-s-e')",
                200,
                &state.to_string(),
            )
            .with_response(
                "https://temper.test/tdata/Files('config-1')/$value",
                200,
                &config,
            )
            .with_response(
                "https://temper.test/tdata/Efforts('effort-1')",
                200,
                &effort.to_string(),
            )
            .with_response(
                "https://temper.test/tdata/ProofPackets('proof-1')",
                200,
                &proof.to_string(),
            )
            .with_response(
                "https://temper.test/tdata/Files('artifact-1')",
                200,
                "{\"Status\":\"Ready\"}",
            )
            .with_response(
                "https://temper.test/tdata/Files('artifact-1')/$value",
                200,
                &artifact.to_string(),
            );
        let result = engine
            .invoke(
                &hash,
                &context("DsfRailwayServiceInstance", module, state.clone()),
                Arc::new(host),
                &WasmResourceLimits::default(),
                Arc::new(RwLock::new(StreamRegistry::default())),
            )
            .await
            .unwrap();
        let expected = if valid {
            "ApplyConfigurationValidationSucceeded"
        } else {
            "ApplyConfigurationValidationFailed"
        };
        assert_eq!(result.callback_action, expected, "{result:?}");
        assert_eq!(result.callback_params["expected_operation_sequence"], 2);
        if valid {
            assert_eq!(
                result.callback_params["intended_configuration"],
                state["request_configuration"]
            );
        }
    }
}
