#[test]
fn live_evolution_requires_immutable_genesis_refs() {
    assert_eq!(
        evolution_ref("MISSING_REF", "demo/agent-answers@seed", false).expect("smoke ref"),
        "demo/agent-answers@seed"
    );
    let error = evolution_ref("MISSING_REF", "demo/agent-answers@seed", true)
        .expect_err("live evolution must reject a label ref");
    assert!(format!("{error:#}").contains("immutable Genesis ref"));
}

#[test]
fn evolution_candidate_changes_are_limited_to_native_bundle_files() {
    assert!(evolution_candidate_path_allowed("specs/answer.ioa.toml"));
    assert!(evolution_candidate_path_allowed("wasm/validator/src/lib.rs"));
    assert!(evolution_candidate_path_allowed("adrs/0002-evidence.md"));
    assert!(!evolution_candidate_path_allowed("crates/random-helper/src/lib.rs"));
    assert!(!evolution_candidate_path_allowed("../evaluator/specs/trial.ioa.toml"));
}

#[test]
fn generic_evolution_plan_allows_subject_defined_metrics_and_traffic() {
    let plan: EvolutionCampaignPlan = serde_json::from_str(
        r#"{"campaign_id":"campaign-support","name":"Support Inbox","director_brief":"Improve resolution.","target_app_ref":"owner/support@1111111111111111111111111111111111111111","evaluator_app_ref":"owner/support-eval@2222222222222222222222222222222222222222","brain_provider":"codex","automation_mode":"automatic_release","traffic_sources":[{"id":"ticket-stream","name":"tickets","kind":"real","description":"incoming support tickets"}],"selection_design":{"id":"support-selection","version_label":"v1","evaluator_namespace":"Acme.SupportEvaluation","trial_suite":{"id":"support-suite","name":"Triage","description":"Resolve urgent cases.","scenario_manifest_json":[{"id":"urgent-ticket"}],"hidden_fixture_locator":"temper://fixture","authored_by":"codex"},"fitness_model_json":{"comparison":"preference","signals":["resolution_quality"]},"constraint_definitions_json":[],"traffic_sources_json":["tickets"],"rationale":"Prefer resolved cases.","proposed_by":"codex","approved_by":"human","metrics":[{"id":"resolution-metric","key":"resolution_quality","description":"quality","instrument_kind":"native","instrument_locator":"temper://quality","interpretation":"higher is better","hard_constraint":false}]},"generations":[{"ordinal":"1","parent_release_ref":"owner/support@1111111111111111111111111111111111111111","selected_app_ref":"owner/support@3333333333333333333333333333333333333333"}],"release_control":{"pause_reason":"inspect","rollback_current_ref":"owner/support@1111111111111111111111111111111111111111","rollback_previous_ref":"owner/support@3333333333333333333333333333333333333333","rollback_reason":"rollback"}}"#,
    )
    .expect("generic campaign plan should parse");
    assert_eq!(plan.selection_design.metrics[0].key, "resolution_quality");
    assert_eq!(plan.selection_design.evaluator_namespace, "Acme.SupportEvaluation");
    assert_eq!(plan.traffic_sources[0].name, "tickets");
}

#[tokio::test]
async fn live_evolution_binds_releases_to_executed_validator_evidence() {
    let _guard = ENV_LOCK.lock().await;
    let path = unique_temp_dir().join("validator-evidence.json");
    fs::create_dir_all(path.parent().expect("manifest parent")).expect("manifest parent");
    fs::write(
        &path,
        r#"{"evaluator_ref":"owner/evaluator@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","records":[{"generation":"1","candidate_ref":"owner/app@1111111111111111111111111111111111111111","status":"Passed","evidence_locator":"temper://trial/g1/validator","result_summary":"generation one passed","measurements":[{"suffix":"quality","traffic_source_id":"simulated","metric_key":"quality","metric_value":"0.8","source_kind":"simulated","evidence_locator":"temper://trial/g1/validator"}]},{"generation":"2","candidate_ref":"owner/app@2222222222222222222222222222222222222222","status":"Passed","evidence_locator":"temper://trial/g2/validator","result_summary":"generation two passed","measurements":[{"suffix":"retention","traffic_source_id":"real","metric_key":"workflow_completion","metric_value":"0.92","source_kind":"real","evidence_locator":"temper://trial/g2/validator"}]}]}"#,
    )
    .expect("validator evidence fixture");
    let _evidence = EnvOverride::set(
        "EVOLUTION_VALIDATOR_EVIDENCE_PATH",
        path.as_os_str().to_os_string(),
    );
    let records = load_evolution_validation_evidence(
        "owner/evaluator@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        &[
            ("1", "owner/app@1111111111111111111111111111111111111111"),
            ("2", "owner/app@2222222222222222222222222222222222222222"),
        ],
        true,
    )
    .expect("matching executed evidence");
    assert_eq!(records[1].measurements[0].metric_key, "workflow_completion");

    let error = load_evolution_validation_evidence(
        "owner/evaluator@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        &[("2", "owner/app@3333333333333333333333333333333333333333")],
        true,
    )
    .expect_err("unexecuted candidate must not release");
    assert!(format!("{error:#}").contains("validator evidence missing generation 2"));
}
