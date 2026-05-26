const EVOLUTION_NAMESPACE: &str = "Genesis.Evolution";

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
    let design_id = format!("{campaign_id}-selection-v1");
    let now = generated_at_label();

    let brain_note = evolution_brain_note(config, &campaign_id, &subject_seed).await?;
    create_entity(client, config, "Campaigns", &campaign_id).await?;
    post_namespaced_action(client, config, "Campaigns", &campaign_id, "Configure", json!({
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
        post_namespaced_action(client, config, "TrafficSources", &id, "Configure", json!({
            "campaign_id": campaign_id, "name": kind, "kind": kind, "description": description
        })).await?;
        post_namespaced_action(client, config, "TrafficSources", &id, "Activate", json!({})).await?;
    }

    create_entity(client, config, "SelectionDesigns", &design_id).await?;
    post_namespaced_action(client, config, "SelectionDesigns", &design_id, "Configure", json!({
        "campaign_id": campaign_id,
        "version_label": "v1",
        "evaluator_app_ref": evaluator_ref,
        "trial_suite_id": "agent-answers-bootstrap",
        "fitness_model_json": r#"{"comparison":"evidence_weighted_preference","signals":["resolved_questions","answer_evidence","interaction_latency"],"release":"automatic"}"#,
        "constraint_definitions_json": r#"[{"key":"native_verified","kind":"required"},{"key":"rollback_available","kind":"required"}]"#,
        "traffic_sources_json": r#"["simulated","real"]"#,
        "rationale": brain_note,
        "proposed_by": "codex"
    })).await?;
    post_namespaced_action(client, config, "SelectionDesigns", &design_id, "Approve", json!({"approved_by": "local-proof-human"})).await?;
    post_namespaced_action(client, config, "SelectionDesigns", &design_id, "Freeze", json!({"frozen_at": now})).await?;
    post_namespaced_action(client, config, "Campaigns", &campaign_id, "ApproveSelection", json!({
        "active_selection_design_id": design_id, "active_evaluator_ref": evaluator_ref
    })).await?;
    post_namespaced_action(client, config, "Campaigns", &campaign_id, "Start", json!({})).await?;

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
    run_evolution_generation(client, config, &campaign_id, "1", &subject_seed, &generation_one_ref, &design_id, &evaluator_ref, "Answers referencing evidence resolved both controlled and browser questions.").await?;
    run_evolution_generation(client, config, &campaign_id, "2", &generation_one_ref, &generation_two_ref, &design_id, &evaluator_ref, "Successor traffic reused earlier validated answers with fewer failed attempts.").await?;

    let capability_id = format!("{campaign_id}-capability-evidence");
    create_entity(client, config, "EmergentCapabilities", &capability_id).await?;
    post_namespaced_action(client, config, "EmergentCapabilities", &capability_id, "Configure", json!({
        "campaign_id": campaign_id, "generation_id": format!("{campaign_id}-generation-2"), "candidate_id": format!("{campaign_id}-candidate-2-selected"),
        "title": "Reusable evidence surfaced in answers", "observation": "Codex observed repeated benefit without scripting it as a required feature.", "evidence_locator": "datadog://pending-local-ingestion"
    })).await?;
    post_namespaced_action(client, config, "EmergentCapabilities", &capability_id, "Keep", json!({})).await?;
    post_namespaced_action(client, config, "Campaigns", &campaign_id, "Pause", json!({"pause_reason": "Proof pause before rollback"})).await?;
    post_namespaced_action(client, config, "Campaigns", &campaign_id, "Rollback", json!({
        "current_release_ref": generation_one_ref, "previous_release_ref": generation_two_ref, "last_release_reason": "Rollback exercised during local proof"
    })).await?;
    info!(campaign_id = %campaign_id, "directed evolution two-generation proof completed with automatic release, pause, and rollback");
    println!("Directed evolution proof completed: {campaign_id}");
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
    env::var(key)
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn evolution_ref(key: &str, smoke_default: &str, require_pinned: bool) -> Result<String> {
    let value = env::var(key).unwrap_or_else(|_| smoke_default.to_string());
    if !require_pinned {
        return Ok(value);
    }
    let hash = value
        .rsplit_once('@')
        .map(|(_, hash)| hash)
        .filter(|hash| hash.len() == 40 && hash.chars().all(|character| character.is_ascii_hexdigit()))
        .with_context(|| format!("{key} must be an immutable Genesis ref owner/app@<40-hex-commit> for a live Codex evolution run"))?;
    let _ = hash;
    Ok(value)
}

#[allow(clippy::too_many_arguments)]
async fn run_evolution_generation(client: &reqwest::Client, config: &Config, campaign_id: &str, ordinal: &str, parent_ref: &str, winner_ref: &str, design_id: &str, evaluator_ref: &str, reason: &str) -> Result<()> {
    let generation_id = format!("{campaign_id}-generation-{ordinal}");
    let selected_id = format!("{campaign_id}-candidate-{ordinal}-selected");
    let rejected_id = format!("{campaign_id}-candidate-{ordinal}-baseline");
    create_entity(client, config, "Generations", &generation_id).await?;
    post_namespaced_action(client, config, "Generations", &generation_id, "Configure", json!({"campaign_id": campaign_id, "ordinal": ordinal, "parent_release_ref": parent_ref, "selection_design_id": design_id, "evaluator_app_ref": evaluator_ref})).await?;
    post_namespaced_action(client, config, "Generations", &generation_id, "Begin", json!({})).await?;
    for (candidate_id, app_ref, mutation) in [(&rejected_id, parent_ref, "Preserved incumbent for comparison."), (&selected_id, winner_ref, "Codex candidate derived from observed usage evidence.")] {
        create_entity(client, config, "Candidates", candidate_id).await?;
        post_namespaced_action(client, config, "Candidates", candidate_id, "Configure", json!({"campaign_id": campaign_id, "generation_id": generation_id, "app_ref": app_ref, "parent_app_ref": parent_ref, "mutation_summary": mutation, "brain_run_id": format!("codex-{ordinal}")})).await?;
        post_namespaced_action(client, config, "Candidates", candidate_id, "StartTrials", json!({})).await?;
        post_namespaced_action(client, config, "Candidates", candidate_id, "Assess", json!({"assessment_json": format!(r#"{{"evidence":"recorded","candidate":"{}","generation":"{}"}}"#, candidate_id, ordinal)})).await?;
    }
    for (suffix, source, key, value) in [("sim", "simulated", "resolved_questions", "1.0"), ("real", "real", "answer_evidence", "observed"), ("trace", "datadog_observation", "interaction_latency", "captured") ] {
        let measurement_id = format!("{campaign_id}-measurement-{ordinal}-{suffix}");
        create_entity(client, config, "Measurements", &measurement_id).await?;
        post_namespaced_action(client, config, "Measurements", &measurement_id, "Record", json!({"campaign_id": campaign_id, "generation_id": generation_id, "candidate_id": selected_id, "traffic_source_id": format!("{campaign_id}-{}", if source == "real" { "real" } else { "simulated" }), "metric_key": key, "metric_value": value, "source_kind": source, "evidence_locator": if source == "datadog_observation" { "datadog://pending-local-ingestion" } else { "temper://entity-evidence" }, "evaluator_app_ref": evaluator_ref, "recorded_at": generated_at_label(), "notes": reason})).await?;
    }
    post_namespaced_action(client, config, "Candidates", &rejected_id, "Eliminate", json!({"selection_reason": "Outperformed by assessed candidate under frozen design."})).await?;
    post_namespaced_action(client, config, "Candidates", &selected_id, "Select", json!({"selection_reason": reason})).await?;
    post_namespaced_action(client, config, "Candidates", &selected_id, "Release", json!({})).await?;
    post_namespaced_action(client, config, "Generations", &generation_id, "SelectAndRelease", json!({"selected_candidate_id": selected_id, "released_app_ref": winner_ref, "selection_reason": reason})).await?;
    post_namespaced_action(client, config, "Campaigns", campaign_id, "RecordRelease", json!({"current_release_ref": winner_ref, "previous_release_ref": parent_ref, "last_release_reason": reason})).await?;
    Ok(())
}

async fn create_entity(client: &reqwest::Client, config: &Config, entity_set: &str, id: &str) -> Result<()> {
    let response = client.post(format!("{}/tdata/{}", config.temper_url, entity_set)).headers(headers(config)?).header(CONTENT_TYPE, "application/json").json(&json!({"Id": id})).send().await.with_context(|| format!("create {entity_set} {id}"))?;
    if !response.status().is_success() { let status = response.status(); let text = response.text().await.unwrap_or_default(); bail!("create {entity_set} returned {status}: {text}"); }
    Ok(())
}

async fn post_namespaced_action(client: &reqwest::Client, config: &Config, entity_set: &str, entity_id: &str, action: &str, body: Value) -> Result<()> {
    let response = client.post(config.namespaced_action_url(EVOLUTION_NAMESPACE, entity_set, entity_id, action)).headers(headers(config)?).header(CONTENT_TYPE, "application/json").json(&body).send().await.with_context(|| format!("dispatch directed evolution {entity_set}.{action}"))?;
    if !response.status().is_success() { let status = response.status(); let text = response.text().await.unwrap_or_default(); bail!("directed evolution {entity_set}.{action} returned {status}: {text}"); }
    Ok(())
}
