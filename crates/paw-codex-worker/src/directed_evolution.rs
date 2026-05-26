const EVOLUTION_NAMESPACE: &str = "Genesis.Evolution";
const EVALUATOR_NAMESPACE: &str = "Genesis.AgentAnswersEvaluation";

#[derive(Debug, Clone, Deserialize)]
struct EvolutionValidationManifest {
    evaluator_ref: String,
    records: Vec<EvolutionValidationEvidence>,
}

#[derive(Debug, Clone, Deserialize)]
struct EvolutionValidationEvidence {
    generation: String,
    candidate_ref: String,
    status: String,
    evidence_locator: String,
    result_summary: String,
    measurements: Vec<EvolutionEvidenceMeasurement>,
}

#[derive(Debug, Clone, Deserialize)]
struct EvolutionEvidenceMeasurement {
    suffix: String,
    traffic_source_id: String,
    metric_key: String,
    metric_value: String,
    source_kind: String,
    evidence_locator: String,
}

#[derive(Debug, Clone, Deserialize)]
struct EvolutionCampaignPlan {
    campaign_id: String,
    name: String,
    director_brief: String,
    target_app_ref: String,
    evaluator_app_ref: String,
    brain_provider: String,
    automation_mode: String,
    traffic_sources: Vec<EvolutionTrafficSourcePlan>,
    selection_design: EvolutionSelectionPlan,
    generations: Vec<EvolutionGenerationPlan>,
    #[serde(default)]
    capabilities: Vec<EvolutionCapabilityPlan>,
    release_control: EvolutionReleaseControlPlan,
}

#[derive(Debug, Clone, Deserialize)]
struct EvolutionTrafficSourcePlan {
    id: String,
    name: String,
    kind: String,
    description: String,
}

#[derive(Debug, Clone, Deserialize)]
struct EvolutionSelectionPlan {
    id: String,
    version_label: String,
    evaluator_namespace: String,
    #[serde(default = "default_trial_suites_entity_set")]
    trial_suites_entity_set: String,
    #[serde(default = "default_metric_definitions_entity_set")]
    metric_definitions_entity_set: String,
    #[serde(default = "default_validator_runs_entity_set")]
    validator_runs_entity_set: String,
    trial_suite: EvolutionTrialSuitePlan,
    fitness_model_json: Value,
    constraint_definitions_json: Value,
    traffic_sources_json: Value,
    rationale: String,
    proposed_by: String,
    approved_by: String,
    metrics: Vec<EvolutionMetricPlan>,
}

#[derive(Debug, Clone, Deserialize)]
struct EvolutionTrialSuitePlan {
    id: String,
    name: String,
    description: String,
    scenario_manifest_json: Value,
    hidden_fixture_locator: String,
    authored_by: String,
}

#[derive(Debug, Clone, Deserialize)]
struct EvolutionMetricPlan {
    id: String,
    key: String,
    description: String,
    instrument_kind: String,
    instrument_locator: String,
    interpretation: String,
    hard_constraint: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct EvolutionGenerationPlan {
    ordinal: String,
    parent_release_ref: String,
    selected_app_ref: String,
    #[serde(default = "default_baseline_mutation")]
    baseline_mutation_summary: String,
    #[serde(default = "default_candidate_mutation")]
    selected_mutation_summary: String,
}

#[derive(Debug, Clone, Deserialize)]
struct EvolutionCapabilityPlan {
    id: String,
    generation_ordinal: String,
    title: String,
    observation: String,
    evidence_locator: String,
    keep: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct EvolutionReleaseControlPlan {
    pause_reason: String,
    rollback_current_ref: String,
    rollback_previous_ref: String,
    rollback_reason: String,
}

async fn run_directed_evolution_demo(client: &reqwest::Client, config: &Config) -> Result<()> {
    let campaign_id = env::var("EVOLUTION_CAMPAIGN_ID")
        .unwrap_or_else(|_| format!("campaign-local-{}", generated_at_label()));
    let require_pinned_refs = evolution_bool_env("EVOLUTION_REQUIRE_PINNED_REFS")
        || evolution_bool_env("PAW_EVOLUTION_USE_CODEX");
    let subject_seed = evolution_ref(
        "EVOLUTION_SUBJECT_SEED_REF",
        "demo/agent-answers@seed",
        require_pinned_refs,
    )?;
    let evaluator_ref = evolution_ref(
        "EVOLUTION_EVALUATOR_REF",
        "demo/agent-answers-evaluation@frozen-v1",
        require_pinned_refs,
    )?;
    let generation_one_ref = evolution_ref(
        "EVOLUTION_GENERATION_ONE_REF",
        "demo/agent-answers@candidate-evidence",
        require_pinned_refs,
    )?;
    let generation_two_ref = evolution_ref(
        "EVOLUTION_GENERATION_TWO_REF",
        "demo/agent-answers@candidate-reuse",
        require_pinned_refs,
    )?;
    let design_id = format!("{campaign_id}-selection-v1");
    let trial_suite_id = format!("{campaign_id}-trial-suite-v1");
    let now = generated_at_label();
    let validation_evidence = load_evolution_validation_evidence(
        &evaluator_ref,
        &[(&"1", &generation_one_ref), (&"2", &generation_two_ref)],
        require_pinned_refs,
    )?;

    let brain_note = evolution_brain_note(config, &campaign_id, &subject_seed).await?;
    create_entity(client, config, "Campaigns", &campaign_id).await?;
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "Campaigns", &campaign_id, "Configure", json!({
        "name": "Agent Answers live evolution proof",
        "director_brief": "Evolve toward useful, evidence-grounded agent answers while preserving rollback and understandable behavior.",
        "target_app_ref": subject_seed,
        "brain_provider": "codex",
        "automation_mode": "automatic_release"
    })).await?;

    for (id, kind, description) in [
        (format!("{campaign_id}-simulated"), "simulated", "Codex actors using controlled questions and validation."),
        (format!("{campaign_id}-real"), "real", "Browser interactions from the installed subject app."),
    ] {
        create_entity(client, config, "TrafficSources", &id).await?;
        post_protocol_action(client, config, EVOLUTION_NAMESPACE, "TrafficSources", &id, "Configure", json!({
            "campaign_id": campaign_id, "name": kind, "kind": kind, "description": description
        })).await?;
        post_protocol_action(client, config, EVOLUTION_NAMESPACE, "TrafficSources", &id, "Activate", json!({})).await?;
    }

    prepare_frozen_evaluator(client, config, &campaign_id, &trial_suite_id, &subject_seed).await?;
    create_entity(client, config, "SelectionDesigns", &design_id).await?;
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "SelectionDesigns", &design_id, "Configure", json!({
        "campaign_id": campaign_id,
        "version_label": "v1",
        "evaluator_app_ref": evaluator_ref,
        "trial_suite_id": trial_suite_id,
        "fitness_model_json": r#"{"comparison":"evidence_weighted_preference","signals":["resolved_questions","answer_evidence","interaction_latency"],"release":"automatic"}"#,
        "constraint_definitions_json": r#"[{"key":"native_verified","kind":"required"},{"key":"rollback_available","kind":"required"}]"#,
        "traffic_sources_json": r#"["simulated","real"]"#,
        "rationale": brain_note,
        "proposed_by": "codex"
    })).await?;
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "SelectionDesigns", &design_id, "Approve", json!({"approved_by": "local-proof-human"})).await?;
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "SelectionDesigns", &design_id, "Freeze", json!({"frozen_at": now})).await?;
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "Campaigns", &campaign_id, "ApproveSelection", json!({
        "active_selection_design_id": design_id, "active_evaluator_ref": evaluator_ref
    })).await?;
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "Campaigns", &campaign_id, "Start", json!({})).await?;

    run_evolution_generation(client, config, &campaign_id, "1", &subject_seed, &generation_one_ref, &design_id, &evaluator_ref, &trial_suite_id, EVALUATOR_NAMESPACE, "ValidatorRuns", "Preserved incumbent for comparison.", "Codex candidate derived from observed usage evidence.", &validation_evidence[0]).await?;
    run_evolution_generation(client, config, &campaign_id, "2", &generation_one_ref, &generation_two_ref, &design_id, &evaluator_ref, &trial_suite_id, EVALUATOR_NAMESPACE, "ValidatorRuns", "Preserved incumbent for comparison.", "Codex candidate derived from observed usage evidence.", &validation_evidence[1]).await?;

    let capability_id = format!("{campaign_id}-capability-evidence");
    create_entity(client, config, "EmergentCapabilities", &capability_id).await?;
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "EmergentCapabilities", &capability_id, "Configure", json!({
        "campaign_id": campaign_id, "generation_id": format!("{campaign_id}-generation-2"), "candidate_id": format!("{campaign_id}-candidate-2-selected"),
        "title": "Reusable evidence surfaced in answers", "observation": "Codex observed repeated benefit without scripting it as a required feature.", "evidence_locator": "datadog://pending-local-ingestion"
    })).await?;
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "EmergentCapabilities", &capability_id, "Keep", json!({})).await?;
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "Campaigns", &campaign_id, "Pause", json!({"pause_reason": "Proof pause before rollback"})).await?;
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "Campaigns", &campaign_id, "Rollback", json!({
        "current_release_ref": generation_one_ref, "previous_release_ref": generation_two_ref, "last_release_reason": "Rollback exercised during local proof"
    })).await?;
    info!(campaign_id = %campaign_id, "directed evolution two-generation proof completed with automatic release, pause, and rollback");
    println!("Directed evolution proof completed: {campaign_id}");
    Ok(())
}

async fn run_directed_evolution_run(client: &reqwest::Client, config: &Config) -> Result<()> {
    let path = required_env("EVOLUTION_CAMPAIGN_PLAN_PATH")?;
    let plan: EvolutionCampaignPlan = serde_json::from_slice(
        &fs::read(&path).with_context(|| format!("read evolution campaign plan {path}"))?,
    )
    .with_context(|| format!("parse evolution campaign plan {path}"))?;
    execute_evolution_campaign_plan(client, config, &plan).await
}

async fn execute_evolution_campaign_plan(client: &reqwest::Client, config: &Config, plan: &EvolutionCampaignPlan) -> Result<()> {
    let require_pinned_refs = evolution_bool_env("EVOLUTION_REQUIRE_PINNED_REFS")
        || evolution_bool_env("PAW_EVOLUTION_USE_CODEX");
    require_evolution_ref("target_app_ref", &plan.target_app_ref, require_pinned_refs)?;
    require_evolution_ref("evaluator_app_ref", &plan.evaluator_app_ref, require_pinned_refs)?;
    if plan.generations.is_empty() {
        bail!("evolution campaign plan must contain at least one generation");
    }
    for generation in &plan.generations {
        require_evolution_ref("parent_release_ref", &generation.parent_release_ref, require_pinned_refs)?;
        require_evolution_ref("selected_app_ref", &generation.selected_app_ref, require_pinned_refs)?;
    }
    let selected_refs: Vec<(&str, &str)> = plan.generations.iter()
        .map(|generation| (generation.ordinal.as_str(), generation.selected_app_ref.as_str()))
        .collect();
    let evidence = load_evolution_validation_evidence(&plan.evaluator_app_ref, &selected_refs, require_pinned_refs)?;
    let rationale = if evolution_bool_env("PAW_EVOLUTION_USE_CODEX") {
        evolution_brain_note(config, &plan.campaign_id, &plan.target_app_ref).await?
    } else {
        plan.selection_design.rationale.clone()
    };
    create_entity(client, config, "Campaigns", &plan.campaign_id).await?;
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "Campaigns", &plan.campaign_id, "Configure", json!({
        "name": plan.name, "director_brief": plan.director_brief, "target_app_ref": plan.target_app_ref,
        "brain_provider": plan.brain_provider, "automation_mode": plan.automation_mode
    })).await?;
    for source in &plan.traffic_sources {
        create_entity(client, config, "TrafficSources", &source.id).await?;
        post_protocol_action(client, config, EVOLUTION_NAMESPACE, "TrafficSources", &source.id, "Configure", json!({
            "campaign_id": plan.campaign_id, "name": source.name, "kind": source.kind, "description": source.description
        })).await?;
        post_protocol_action(client, config, EVOLUTION_NAMESPACE, "TrafficSources", &source.id, "Activate", json!({})).await?;
    }
    prepare_frozen_evaluator_plan(client, config, plan).await?;
    let selection = &plan.selection_design;
    create_entity(client, config, "SelectionDesigns", &selection.id).await?;
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "SelectionDesigns", &selection.id, "Configure", json!({
        "campaign_id": plan.campaign_id, "version_label": selection.version_label, "evaluator_app_ref": plan.evaluator_app_ref,
        "trial_suite_id": selection.trial_suite.id, "fitness_model_json": selection.fitness_model_json.to_string(),
        "constraint_definitions_json": selection.constraint_definitions_json.to_string(), "traffic_sources_json": selection.traffic_sources_json.to_string(),
        "rationale": rationale, "proposed_by": selection.proposed_by
    })).await?;
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "SelectionDesigns", &selection.id, "Approve", json!({"approved_by": selection.approved_by})).await?;
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "SelectionDesigns", &selection.id, "Freeze", json!({"frozen_at": generated_at_label()})).await?;
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "Campaigns", &plan.campaign_id, "ApproveSelection", json!({
        "active_selection_design_id": selection.id, "active_evaluator_ref": plan.evaluator_app_ref
    })).await?;
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "Campaigns", &plan.campaign_id, "Start", json!({})).await?;
    for (generation, record) in plan.generations.iter().zip(&evidence) {
        run_evolution_generation(client, config, &plan.campaign_id, &generation.ordinal, &generation.parent_release_ref, &generation.selected_app_ref, &selection.id, &plan.evaluator_app_ref, &selection.trial_suite.id, &selection.evaluator_namespace, &selection.validator_runs_entity_set, &generation.baseline_mutation_summary, &generation.selected_mutation_summary, record).await?;
    }
    for capability in &plan.capabilities {
        create_entity(client, config, "EmergentCapabilities", &capability.id).await?;
        post_protocol_action(client, config, EVOLUTION_NAMESPACE, "EmergentCapabilities", &capability.id, "Configure", json!({
            "campaign_id": plan.campaign_id, "generation_id": format!("{}-generation-{}", plan.campaign_id, capability.generation_ordinal),
            "candidate_id": format!("{}-candidate-{}-selected", plan.campaign_id, capability.generation_ordinal), "title": capability.title,
            "observation": capability.observation, "evidence_locator": capability.evidence_locator
        })).await?;
        let action = if capability.keep { "Keep" } else { "Reject" };
        post_protocol_action(client, config, EVOLUTION_NAMESPACE, "EmergentCapabilities", &capability.id, action, json!({})).await?;
    }
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "Campaigns", &plan.campaign_id, "Pause", json!({"pause_reason": plan.release_control.pause_reason})).await?;
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "Campaigns", &plan.campaign_id, "Rollback", json!({
        "current_release_ref": plan.release_control.rollback_current_ref, "previous_release_ref": plan.release_control.rollback_previous_ref,
        "last_release_reason": plan.release_control.rollback_reason
    })).await?;
    info!(campaign_id = %plan.campaign_id, "directed evolution campaign plan completed with automatic release, pause, and rollback");
    println!("Directed evolution campaign completed: {}", plan.campaign_id);
    Ok(())
}

async fn run_directed_evolution_mutation(config: &Config) -> Result<()> {
    let candidate_dir = PathBuf::from(required_env("EVOLUTION_CANDIDATE_DIR")?);
    let direction = required_env("EVOLUTION_DIRECTION")?;
    let generation = env::var("EVOLUTION_GENERATION_ORDINAL").unwrap_or_else(|_| "next".to_string());
    let parent_ref = env::var("EVOLUTION_PARENT_REF").unwrap_or_else(|_| "local parent".to_string());
    if !candidate_dir.join("app.toml").is_file() || !candidate_dir.join("specs").is_dir() {
        bail!("EVOLUTION_CANDIDATE_DIR must be a Temper-native app bundle with app.toml and specs/");
    }
    let validator_contract = env::var("EVOLUTION_VALIDATOR_CONTRACT")
        .unwrap_or_else(|_| "Preserve every behavior required by the frozen evaluator contract supplied for this campaign; additive behavior is allowed.".to_string());
    let prompt = format!(
        "You are the Codex v1 mutation brain for directed evolution. Edit the Temper-native app bundle in the current directory for generation {generation}, derived from {parent_ref}. Human direction: {direction}. Frozen evaluator contract: {validator_contract}. Produce one small, coherent app improvement grounded in that direction. Only edit app-native files: app.toml, APP.md, specs/, policies/, wasm/, content/, seed-data/, and adrs/. Keep the app installable and update specs, CSDL, policies, and ADRs together when behavior changes. Do not edit evaluator files, create external crates, run git commands, or invent fitness results."
    );
    let output = run_codex_exec_command(config, &candidate_dir, prompt, "generate directed evolution candidate").await?;
    if !output.status.success() {
        bail!("directed evolution mutation failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    let status = Command::new("git")
        .args(["-C", candidate_dir.to_str().context("candidate path utf-8")?, "status", "--porcelain"])
        .output()
        .await
        .context("inspect generated candidate status")?;
    if !status.status.success() {
        bail!("could not inspect generated candidate: {}", String::from_utf8_lossy(&status.stderr));
    }
    let changed: Vec<String> = String::from_utf8_lossy(&status.stdout)
        .lines()
        .filter_map(|line| line.get(3..).map(str::trim).map(str::to_string))
        .filter(|path| !path.is_empty())
        .collect();
    if changed.is_empty() {
        bail!("Codex mutation completed without producing a candidate change");
    }
    if let Some(path) = changed.iter().find(|path| !evolution_candidate_path_allowed(path)) {
        bail!("Codex mutation changed non-native candidate path '{path}'");
    }
    println!("Directed evolution candidate generated for generation {generation}: {}", changed.join(", "));
    Ok(())
}

fn evolution_candidate_path_allowed(path: &str) -> bool {
    path == "app.toml"
        || path == "APP.md"
        || ["specs/", "policies/", "wasm/", "content/", "seed-data/", "adrs/"]
            .iter()
            .any(|prefix| path.starts_with(prefix))
}

fn default_baseline_mutation() -> String {
    "Retained incumbent candidate for comparison.".to_string()
}

fn default_candidate_mutation() -> String {
    "Candidate proposed by the configured evolution brain.".to_string()
}

fn default_trial_suites_entity_set() -> String {
    "TrialSuites".to_string()
}

fn default_metric_definitions_entity_set() -> String {
    "MetricDefinitions".to_string()
}

fn default_validator_runs_entity_set() -> String {
    "ValidatorRuns".to_string()
}

fn load_evolution_validation_evidence(
    evaluator_ref: &str,
    selected_refs: &[(&str, &str)],
    required: bool,
) -> Result<Vec<EvolutionValidationEvidence>> {
    let path = match env::var("EVOLUTION_VALIDATOR_EVIDENCE_PATH") {
        Ok(path) if !path.trim().is_empty() => path,
        _ if required => {
            bail!("live directed evolution requires EVOLUTION_VALIDATOR_EVIDENCE_PATH from executed frozen-evaluator trials")
        }
        _ => {
            return Ok(selected_refs
                .iter()
                .map(|(generation, candidate_ref)| EvolutionValidationEvidence {
                    generation: (*generation).to_string(),
                    candidate_ref: (*candidate_ref).to_string(),
                    status: "Passed".to_string(),
                    evidence_locator: format!("temper://fixture/generation-{generation}"),
                    result_summary: "Fixture-only protocol evidence.".to_string(),
                    measurements: vec![EvolutionEvidenceMeasurement {
                        suffix: "fixture".to_string(),
                        traffic_source_id: "fixture".to_string(),
                        metric_key: "fixture_result".to_string(),
                        metric_value: "passed".to_string(),
                        source_kind: "fixture".to_string(),
                        evidence_locator: format!("temper://fixture/generation-{generation}"),
                    }],
                })
                .collect());
        }
    };
    let manifest: EvolutionValidationManifest = serde_json::from_slice(
        &fs::read(&path).with_context(|| format!("read validator evidence manifest {path}"))?,
    )
    .with_context(|| format!("parse validator evidence manifest {path}"))?;
    if manifest.evaluator_ref != evaluator_ref {
        bail!("validator evidence evaluator ref does not match the frozen evaluator ref");
    }
    let mut selected = Vec::with_capacity(selected_refs.len());
    for (generation, candidate_ref) in selected_refs {
        let evidence = manifest
            .records
            .iter()
            .find(|record| record.generation == *generation && record.candidate_ref == *candidate_ref)
            .with_context(|| format!("validator evidence missing generation {generation} candidate {candidate_ref}"))?;
        if evidence.status != "Passed" || evidence.evidence_locator.trim().is_empty() || evidence.measurements.is_empty()
            || evidence.measurements.iter().any(|measurement| measurement.metric_key.trim().is_empty() || measurement.metric_value.trim().is_empty() || measurement.source_kind.trim().is_empty() || measurement.evidence_locator.trim().is_empty()) {
            bail!("generation {generation} has no passing executed validator evidence");
        }
        selected.push(evidence.clone());
    }
    Ok(selected)
}

async fn prepare_frozen_evaluator_plan(client: &reqwest::Client, config: &Config, plan: &EvolutionCampaignPlan) -> Result<()> {
    let suite = &plan.selection_design.trial_suite;
    let selection = &plan.selection_design;
    create_entity(client, config, &selection.trial_suites_entity_set, &suite.id).await?;
    post_protocol_action(client, config, &selection.evaluator_namespace, &selection.trial_suites_entity_set, &suite.id, "Configure", json!({
        "name": suite.name, "description": suite.description, "subject_app_ref": plan.target_app_ref,
        "scenario_manifest_json": suite.scenario_manifest_json.to_string(), "hidden_fixture_locator": suite.hidden_fixture_locator,
        "authored_by": suite.authored_by
    })).await?;
    post_protocol_action(client, config, &selection.evaluator_namespace, &selection.trial_suites_entity_set, &suite.id, "Freeze", json!({"frozen_at": generated_at_label()})).await?;
    for metric in &selection.metrics {
        create_entity(client, config, &selection.metric_definitions_entity_set, &metric.id).await?;
        post_protocol_action(client, config, &selection.evaluator_namespace, &selection.metric_definitions_entity_set, &metric.id, "Configure", json!({
            "trial_suite_id": suite.id, "key": metric.key, "description": metric.description, "instrument_kind": metric.instrument_kind,
            "instrument_locator": metric.instrument_locator, "interpretation": metric.interpretation, "hard_constraint": metric.hard_constraint
        })).await?;
        post_protocol_action(client, config, &selection.evaluator_namespace, &selection.metric_definitions_entity_set, &metric.id, "Freeze", json!({"frozen_at": generated_at_label()})).await?;
    }
    Ok(())
}

async fn prepare_frozen_evaluator(
    client: &reqwest::Client,
    config: &Config,
    campaign_id: &str,
    trial_suite_id: &str,
    subject_ref: &str,
) -> Result<()> {
    create_entity(client, config, "TrialSuites", trial_suite_id).await?;
    post_protocol_action(client, config, EVALUATOR_NAMESPACE, "TrialSuites", trial_suite_id, "Configure", json!({
        "name": "Agent Answers bootstrap behavior",
        "description": "Frozen native trial suite covering question resolution, evidence visibility, and successor reuse.",
        "subject_app_ref": subject_ref,
        "scenario_manifest_json": r#"[{"id":"controlled-question","traffic":"simulated"},{"id":"browser-answer","traffic":"real"},{"id":"successor-reuse","traffic":"simulated"}]"#,
        "hidden_fixture_locator": format!("temper://campaigns/{campaign_id}/fixtures/bootstrap"),
        "authored_by": "codex-with-human-approval"
    })).await?;
    post_protocol_action(client, config, EVALUATOR_NAMESPACE, "TrialSuites", trial_suite_id, "Freeze", json!({"frozen_at": generated_at_label()})).await?;
    for (suffix, key, description, kind, locator, hard) in [
        ("resolved", "resolved_questions", "Controlled questions resolve after accepted answers.", "native_validator", "temper://validators/question-resolution", true),
        ("evidence", "answer_evidence", "Real usage exposes an evidence locator on accepted answers.", "native_validator", "temper://validators/answer-evidence", false),
        ("latency", "interaction_latency", "Observed interaction latency remains inspectable in Datadog.", "datadog", "datadog://pending-local-ingestion", false),
    ] {
        let metric_id = format!("{campaign_id}-metric-{suffix}");
        create_entity(client, config, "MetricDefinitions", &metric_id).await?;
        post_protocol_action(client, config, EVALUATOR_NAMESPACE, "MetricDefinitions", &metric_id, "Configure", json!({
            "trial_suite_id": trial_suite_id, "key": key, "description": description, "instrument_kind": kind,
            "instrument_locator": locator, "interpretation": "Evidence contributes to candidate comparison under the approved selection design.", "hard_constraint": hard
        })).await?;
        post_protocol_action(client, config, EVALUATOR_NAMESPACE, "MetricDefinitions", &metric_id, "Freeze", json!({"frozen_at": generated_at_label()})).await?;
    }
    Ok(())
}

async fn evolution_brain_note(config: &Config, campaign_id: &str, subject_seed: &str) -> Result<String> {
    if !evolution_bool_env("PAW_EVOLUTION_USE_CODEX") {
        return Ok("Codex-backed adapter configured. Smoke mode uses a stable approved design so protocol tests remain reproducible.".to_string());
    }
    let prompt = format!("You are the Codex brain proposing a directed-evolution selection design for campaign {campaign_id}. The subject is the Temper-native app ref {subject_seed}. Return one concise rationale sentence only. Use both simulated and real usage evidence; preserve native verification, frozen evaluator isolation, automatic release visibility, pause, and rollback. Do not prescribe a feature mutation.");
    let output = run_codex_exec_command(config, &config.repo_root, prompt, "run directed evolution Codex brain").await?;
    if !output.status.success() {
        bail!("directed evolution Codex brain failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().chars().take(1200).collect())
}

fn evolution_bool_env(key: &str) -> bool {
    env::var(key).map(|value| value == "1" || value.eq_ignore_ascii_case("true")).unwrap_or(false)
}

fn evolution_ref(key: &str, smoke_default: &str, require_pinned: bool) -> Result<String> {
    let value = env::var(key).unwrap_or_else(|_| smoke_default.to_string());
    require_evolution_ref(key, &value, require_pinned)?;
    Ok(value)
}

fn require_evolution_ref(label: &str, value: &str, require_pinned: bool) -> Result<()> {
    if !require_pinned { return Ok(()); }
    value.rsplit_once('@').map(|(_, hash)| hash)
        .filter(|hash| hash.len() == 40 && hash.chars().all(|character| character.is_ascii_hexdigit()))
        .with_context(|| format!("{label} must be an immutable Genesis ref owner/app@<40-hex-commit> for a live Codex evolution run"))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run_evolution_generation(client: &reqwest::Client, config: &Config, campaign_id: &str, ordinal: &str, parent_ref: &str, winner_ref: &str, design_id: &str, evaluator_ref: &str, trial_suite_id: &str, evaluator_namespace: &str, validator_runs_entity_set: &str, baseline_mutation: &str, selected_mutation: &str, evidence: &EvolutionValidationEvidence) -> Result<()> {
    let generation_id = format!("{campaign_id}-generation-{ordinal}");
    let selected_id = format!("{campaign_id}-candidate-{ordinal}-selected");
    let rejected_id = format!("{campaign_id}-candidate-{ordinal}-baseline");
    create_entity(client, config, "Generations", &generation_id).await?;
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "Generations", &generation_id, "Configure", json!({"campaign_id": campaign_id, "ordinal": ordinal, "parent_release_ref": parent_ref, "selection_design_id": design_id, "evaluator_app_ref": evaluator_ref})).await?;
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "Generations", &generation_id, "Begin", json!({})).await?;
    for (candidate_id, app_ref, mutation) in [(&rejected_id, parent_ref, baseline_mutation), (&selected_id, winner_ref, selected_mutation)] {
        create_entity(client, config, "Candidates", candidate_id).await?;
        post_protocol_action(client, config, EVOLUTION_NAMESPACE, "Candidates", candidate_id, "Configure", json!({"campaign_id": campaign_id, "generation_id": generation_id, "app_ref": app_ref, "parent_app_ref": parent_ref, "mutation_summary": mutation, "brain_run_id": format!("codex-{ordinal}")})).await?;
        post_protocol_action(client, config, EVOLUTION_NAMESPACE, "Candidates", candidate_id, "StartTrials", json!({})).await?;
        post_protocol_action(client, config, EVOLUTION_NAMESPACE, "Candidates", candidate_id, "Assess", json!({"assessment_json": format!(r#"{{"evidence":"validator-run","candidate":"{}","generation":"{}"}}"#, candidate_id, ordinal)})).await?;
    }
    let validator_id = format!("{campaign_id}-validator-{ordinal}-selected");
    create_entity(client, config, validator_runs_entity_set, &validator_id).await?;
    post_protocol_action(client, config, evaluator_namespace, validator_runs_entity_set, &validator_id, "Configure", json!({
        "trial_suite_id": trial_suite_id, "candidate_id": selected_id, "scenario_id": format!("generation-{ordinal}-mixed-traffic"), "validator_kind": "native_trial"
    })).await?;
    post_protocol_action(client, config, evaluator_namespace, validator_runs_entity_set, &validator_id, "Pass", json!({
        "evidence_locator": evidence.evidence_locator, "result_summary": evidence.result_summary
    })).await?;
    for measurement in &evidence.measurements {
        let measurement_id = format!("{campaign_id}-measurement-{ordinal}-{}", measurement.suffix);
        let traffic_source_id = measurement.traffic_source_id.replace("{campaign_id}", campaign_id);
        create_entity(client, config, "Measurements", &measurement_id).await?;
        post_protocol_action(client, config, EVOLUTION_NAMESPACE, "Measurements", &measurement_id, "Record", json!({"campaign_id": campaign_id, "generation_id": generation_id, "candidate_id": selected_id, "traffic_source_id": traffic_source_id, "metric_key": measurement.metric_key, "metric_value": measurement.metric_value, "source_kind": measurement.source_kind, "evidence_locator": measurement.evidence_locator, "evaluator_app_ref": evaluator_ref, "recorded_at": generated_at_label(), "notes": evidence.result_summary})).await?;
    }
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "Candidates", &rejected_id, "Eliminate", json!({"selection_reason": "Outperformed by assessed candidate under frozen design."})).await?;
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "Candidates", &selected_id, "Select", json!({"selection_reason": evidence.result_summary})).await?;
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "Candidates", &selected_id, "Release", json!({})).await?;
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "Generations", &generation_id, "SelectAndRelease", json!({"selected_candidate_id": selected_id, "released_app_ref": winner_ref, "selection_reason": evidence.result_summary})).await?;
    post_protocol_action(client, config, EVOLUTION_NAMESPACE, "Campaigns", campaign_id, "RecordRelease", json!({"current_release_ref": winner_ref, "previous_release_ref": parent_ref, "last_release_reason": evidence.result_summary})).await?;
    Ok(())
}

async fn create_entity(client: &reqwest::Client, config: &Config, entity_set: &str, id: &str) -> Result<()> {
    let response = client.post(format!("{}/tdata/{}", config.temper_url, entity_set)).headers(headers(config)?).header(CONTENT_TYPE, "application/json").json(&json!({"Id": id})).send().await.with_context(|| format!("create {entity_set} {id}"))?;
    if !response.status().is_success() { let status = response.status(); let text = response.text().await.unwrap_or_default(); bail!("create {entity_set} returned {status}: {text}"); }
    Ok(())
}

async fn post_protocol_action(client: &reqwest::Client, config: &Config, namespace: &str, entity_set: &str, entity_id: &str, action: &str, body: Value) -> Result<()> {
    let response = client.post(config.namespaced_action_url(namespace, entity_set, entity_id, action)).headers(headers(config)?).header(CONTENT_TYPE, "application/json").json(&body).send().await.with_context(|| format!("dispatch {namespace} {entity_set}.{action}"))?;
    if !response.status().is_success() { let status = response.status(); let text = response.text().await.unwrap_or_default(); bail!("{namespace} {entity_set}.{action} returned {status}: {text}"); }
    Ok(())
}
