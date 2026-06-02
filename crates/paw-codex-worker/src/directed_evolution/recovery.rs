async fn recover_pending_directed_evolution_stage_results(
    client: &reqwest::Client,
    config: &Config,
) -> Result<()> {
    let stage_results =
        query_directed_evolution_rows(client, config, "StageResults", "Status eq 'Pending'", 300)
            .await?;
    let trials = query_directed_evolution_rows(client, config, "Trials", "", 1000).await?;
    let director_headers = directed_evolution_director_headers(config)?;

    for stage_result in stage_results {
        let stage_result_id = directed_evolution_row_id(&stage_result);
        let fields = directed_evolution_row_fields(&stage_result);
        let episode_id = value_field_string(&fields, &["EpisodeId", "episode_id"]);
        let generation_id = value_field_string(&fields, &["GenerationId", "generation_id"]);
        let variant_id = value_field_string(&fields, &["VariantId", "variant_id"]);
        let stage_id = value_field_string(&fields, &["EvaluationStageId", "evaluation_stage_id"]);
        if stage_result_id.is_empty()
            || episode_id.is_empty()
            || generation_id.is_empty()
            || variant_id.is_empty()
            || stage_id.is_empty()
        {
            continue;
        }

        let stage_fields =
            fetch_directed_evolution_entity_fields(client, config, "EvaluationStages", &stage_id)
                .await?;
        let stage_kind = value_field_string(&stage_fields, &["StageKind", "stage_kind"])
            .to_ascii_lowercase();
        let role = recovered_stage_role(&stage_fields);
        let ready = if stage_kind.contains("simulated") {
            directed_evolution_trials_terminal_for_stage(&trials, &stage_result_id)
        } else if role == "telemetry_evaluator" {
            directed_evolution_trials_terminal_for_variant(&trials, &variant_id)
        } else {
            false
        };
        if !ready {
            continue;
        }
        if recovered_stage_work_item_exists(client, config, &stage_result_id, &role).await? {
            continue;
        }

        let variant_fields =
            fetch_directed_evolution_entity_fields(client, config, "Variants", &variant_id)
                .await?;
        if value_field_string(&variant_fields, &["Status", "status"]) != "Active" {
            continue;
        }

        let work_item_id = create_directed_evolution_entity_with_headers(
            client,
            config,
            &director_headers,
            "WorkItems",
        )
        .await?;
        let prompt = recovered_stage_prompt(RecoveredStagePrompt {
            role: &role,
            episode_id: &episode_id,
            generation_id: &generation_id,
            variant_id: &variant_id,
            stage_id: &stage_id,
            stage_result_id: &stage_result_id,
            work_item_id: &work_item_id,
            stage_fields: &stage_fields,
            variant_fields: &variant_fields,
            trials: &trials,
        });
        if let Err(error) = post_directed_evolution_action_with_headers(
            client,
            config,
            &director_headers,
            "StageResults",
            &stage_result_id,
            "StartStageResult",
            json!({
                "EpisodeId": episode_id,
                "GenerationId": generation_id,
                "VariantId": variant_id,
                "EvaluationStageId": stage_id,
                "WorkItemId": work_item_id,
            }),
        )
        .await
        {
            let cancel_reason =
                format!("recovery skipped StageResult start because it changed state: {error}");
            if let Err(cancel_error) = post_paw_orchestration_action_with_headers(
                client,
                config,
                &director_headers,
                "WorkItems",
                &work_item_id,
                "CancelWorkItem",
                json!({ "Reason": cancel_reason }),
            )
            .await
            {
                warn!(
                    stage_result_id,
                    work_item_id,
                    %cancel_error,
                    "could not cancel placeholder Directed Evolution recovery WorkItem"
                );
            }
            info!(
                stage_result_id,
                work_item_id,
                role,
                %error,
                "skipped Directed Evolution stage-result recovery because another actor changed the stage"
            );
            continue;
        }
        post_paw_orchestration_action_with_headers(
            client,
            config,
            &director_headers,
            "WorkItems",
            &work_item_id,
            "QueueWorkItem",
            json!({
                "Role": role,
                "TargetEntityType": "StageResult",
                "TargetEntityId": stage_result_id,
                "PromptRef": format!("literal:{prompt}"),
                "ContextRef": format!("stage_result:{stage_result_id}"),
                "OutputSchemaRef": "directed-evolution.stage-evaluation.v1",
                "CorrelationJson": json!({
                    "episode_id": episode_id,
                    "generation_id": generation_id,
                    "variant_id": variant_id,
                    "evaluation_stage_id": stage_id,
                    "stage_result_id": stage_result_id,
                    "role": role,
                    "recovered_after_terminal_trials": true,
                })
                .to_string(),
            }),
        )
        .await?;
        info!(
            stage_result_id,
            work_item_id,
            role,
            "recovered pending Directed Evolution stage result after terminal trials"
        );
    }

    Ok(())
}

fn recovered_stage_role(stage_fields: &Value) -> String {
    let executor = value_field_string(stage_fields, &["ExecutorKind", "executor_kind"]);
    if directed_evolution_stage_evaluator_role(&executor) {
        return executor;
    }
    let kind = value_field_string(stage_fields, &["StageKind", "stage_kind"]).to_ascii_lowercase();
    let provenance =
        value_field_string(stage_fields, &["MeasurementProvenance", "measurement_provenance"])
            .to_ascii_lowercase();
    if kind.contains("telemetry") || kind.contains("datadog") || provenance.contains("datadog") {
        "telemetry_evaluator".to_string()
    } else if kind.contains("simulated") {
        "viability_evaluator".to_string()
    } else {
        "reviewer".to_string()
    }
}

fn directed_evolution_trials_terminal_for_stage(trials: &[Value], stage_result_id: &str) -> bool {
    let matching = trials
        .iter()
        .filter(|trial| {
            value_field_string(
                &directed_evolution_row_fields(trial),
                &["StageResultId", "stage_result_id"],
            ) == stage_result_id
        })
        .collect::<Vec<_>>();
    !matching.is_empty() && matching.iter().all(directed_evolution_trial_terminal)
}

fn directed_evolution_trials_terminal_for_variant(trials: &[Value], variant_id: &str) -> bool {
    let matching = trials
        .iter()
        .filter(|trial| {
            value_field_string(
                &directed_evolution_row_fields(trial),
                &["VariantId", "variant_id"],
            ) == variant_id
        })
        .collect::<Vec<_>>();
    !matching.is_empty() && matching.iter().all(directed_evolution_trial_terminal)
}

fn directed_evolution_trial_terminal(trial: &&Value) -> bool {
    matches!(
        value_field_string(
            &directed_evolution_row_fields(trial),
            &["Status", "status"]
        )
        .as_str(),
        "Succeeded" | "Failed" | "Archived"
    )
}

async fn recovered_stage_work_item_exists(
    client: &reqwest::Client,
    config: &Config,
    stage_result_id: &str,
    role: &str,
) -> Result<bool> {
    let work_items = query_directed_evolution_rows(client, config, "WorkItems", "", 1000).await?;
    Ok(work_items.iter().any(|item| {
        let fields = directed_evolution_row_fields(item);
        value_field_string(&fields, &["TargetEntityType", "target_entity_type"]) == "StageResult"
            && value_field_string(&fields, &["TargetEntityId", "target_entity_id"]) == stage_result_id
            && value_field_string(&fields, &["Role", "role"]) == role
            && matches!(
                value_field_string(&fields, &["Status", "status"]).as_str(),
                "Queued" | "Claimed" | "Running" | "Succeeded"
            )
    }))
}

struct RecoveredStagePrompt<'a> {
    role: &'a str,
    episode_id: &'a str,
    generation_id: &'a str,
    variant_id: &'a str,
    stage_id: &'a str,
    stage_result_id: &'a str,
    work_item_id: &'a str,
    stage_fields: &'a Value,
    variant_fields: &'a Value,
    trials: &'a [Value],
}

fn recovered_stage_prompt(args: RecoveredStagePrompt<'_>) -> String {
    let stage_name = value_field_string(args.stage_fields, &["StageName", "stage_name"]);
    let stage_kind = value_field_string(args.stage_fields, &["StageKind", "stage_kind"]);
    let app_ref = value_field_string(args.variant_fields, &["AppRef", "app_ref"]);
    let runtime_ref = value_field_string(args.variant_fields, &["RuntimeRef", "runtime_ref"]);
    let variant_summary = value_field_string(args.variant_fields, &["Summary", "summary"]);
    let trial_context = recovered_trial_context(args.trials, args.variant_id);
    let header_block = format!(
        concat!(
            "x-de-episode-id: {episode_id}\n",
            "x-de-generation-id: {generation_id}\n",
            "x-de-variant-id: {variant_id}\n",
            "x-de-stage-id: {stage_id}\n",
            "x-de-stage-result-id: {stage_result_id}\n",
            "x-de-work-item-id: {work_item_id}\n",
            "x-de-runtime-ref: {runtime_ref}\n",
            "x-de-app-ref: {app_ref}",
        ),
        episode_id = args.episode_id,
        generation_id = args.generation_id,
        variant_id = args.variant_id,
        stage_id = args.stage_id,
        stage_result_id = args.stage_result_id,
        work_item_id = args.work_item_id,
        runtime_ref = runtime_ref,
        app_ref = app_ref
    );

    if args.role == "telemetry_evaluator" {
        return format!(
            concat!(
                "Evaluate Directed Evolution Datadog telemetry after simulated users completed.\n",
                "EpisodeId: {episode_id}\n",
                "GenerationId: {generation_id}\n",
                "VariantId: {variant_id}\n",
                "EvaluationStageId: {stage_id}\n",
                "StageResultId: {stage_result_id}\n",
                "StageName: {stage_name}\n",
                "StageKind: {stage_kind}\n",
                "TemperApiBase: {base}\n",
                "AppRef: {app_ref}\n",
                "RuntimeRef: {runtime_ref}\n",
                "VariantSummary: {variant_summary}\n\n",
                "RecordedTrialEvidence:\n{trial_context}\n\n",
                "DirectedEvolutionHeaders:\n{header_block}\n\n",
                "This is a Datadog-measured telemetry stage. Use Datadog MCP aggregate/SQL evidence over brittle log-explorer field syntax: filter for \"directed evolution runtime request\", select directed_evolution.episode_id, directed_evolution.variant_id, and tenant, and count rows for this exact episode, variant, and runtime tenant parsed from RuntimeRef. Return provenance_kind=datadog-measured. The first evidence_scope item must include query, time_window, result_count, interpretation, zero_result_meaning, and datadog_url. Zero matching runtime-request logs is failure; runtime probes may support but must not replace Datadog. Return exactly one concise JSON object with passed/status/summary/failure_reason/evaluator_role/provenance_kind/metrics/decision_basis/inputs/evidence_scope/evidence_refs/reasoning_summary.",
            ),
            base = configless_public_temper_api_base(),
            episode_id = args.episode_id,
            generation_id = args.generation_id,
            variant_id = args.variant_id,
            stage_id = args.stage_id,
            stage_result_id = args.stage_result_id,
            stage_name = stage_name,
            stage_kind = stage_kind,
            app_ref = app_ref,
            runtime_ref = runtime_ref,
            variant_summary = variant_summary,
            trial_context = trial_context,
            header_block = header_block
        );
    }

    format!(
        concat!(
            "Evaluate the completed simulated-user trial observations for a Directed Evolution variant.\n",
            "EpisodeId: {episode_id}\n",
            "GenerationId: {generation_id}\n",
            "VariantId: {variant_id}\n",
            "EvaluationStageId: {stage_id}\n",
            "StageResultId: {stage_result_id}\n",
            "StageName: {stage_name}\n",
            "StageKind: {stage_kind}\n",
            "TemperApiBase: {base}\n",
            "AppRef: {app_ref}\n",
            "RuntimeRef: {runtime_ref}\n",
            "VariantSummary: {variant_summary}\n\n",
            "RecordedTrialEvidence:\n{trial_context}\n\n",
            "Use only recorded simulated-user observations, blockers, friction, and live-runtime evidence. Do not query Datadog for this non-telemetry stage, do not select a winner, and do not modify files or evaluators. Return exactly one concise JSON object with passed/status/summary/failure_reason/evaluator_role=viability_evaluator/provenance_kind=brain-judged/metrics/decision_basis/inputs/evidence_scope/evidence_refs/reasoning_summary.",
        ),
        base = configless_public_temper_api_base(),
        episode_id = args.episode_id,
        generation_id = args.generation_id,
        variant_id = args.variant_id,
        stage_id = args.stage_id,
        stage_result_id = args.stage_result_id,
        stage_name = stage_name,
        stage_kind = stage_kind,
        app_ref = app_ref,
        runtime_ref = runtime_ref,
        variant_summary = variant_summary,
        trial_context = trial_context
    )
}

fn recovered_trial_context(trials: &[Value], variant_id: &str) -> String {
    let mut rows = Vec::new();
    for trial in trials {
        let fields = directed_evolution_row_fields(trial);
        if value_field_string(&fields, &["VariantId", "variant_id"]) != variant_id {
            continue;
        }
        rows.push(format!(
            "- {} {} run={} summary={} blocker={} intent={}",
            directed_evolution_row_id(trial),
            value_field_string(&fields, &["Status", "status"]),
            value_field_string(&fields, &["RunIndex", "run_index"]),
            value_field_string(&fields, &["Summary", "summary", "FailureReason", "failure_reason"]),
            value_field_string(&fields, &["Blocker", "blocker"]),
            value_field_string(&fields, &["IntentSatisfied", "intent_satisfied"]),
        ));
    }
    if rows.is_empty() {
        "No trial evidence found for this variant.".to_string()
    } else {
        rows.join("\n")
    }
}

fn configless_public_temper_api_base() -> String {
    env::var("DIRECTED_EVOLUTION_PUBLIC_API_URL")
        .ok()
        .or_else(|| env::var("DIRECTED_EVOLUTION_GENESIS_URL").ok())
        .or_else(|| env::var("TEMPER_URL").ok())
        .unwrap_or_else(|| "https://genesis-production-164d.up.railway.app".to_string())
}

async fn query_directed_evolution_rows(
    client: &reqwest::Client,
    config: &Config,
    entity_set: &str,
    filter: &str,
    top: usize,
) -> Result<Vec<Value>> {
    let mut url = format!("{}/tdata/{entity_set}?$top={top}", config.temper_url);
    if !filter.trim().is_empty() {
        url.push_str("&$filter=");
        url.push_str(&filter.replace(' ', "%20").replace('\'', "%27"));
    }
    let response = client
        .get(url)
        .headers(headers(config)?)
        .send()
        .await
        .with_context(|| format!("query Directed Evolution {entity_set}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        bail!("query Directed Evolution {entity_set} returned {status}: {text}");
    }
    let body: Value = response.json().await.context("parse Directed Evolution query")?;
    Ok(body
        .get("value")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

fn directed_evolution_row_fields(row: &Value) -> Value {
    row.get("fields").cloned().unwrap_or_else(|| json!({}))
}

fn directed_evolution_row_id(row: &Value) -> String {
    let fields = directed_evolution_row_fields(row);
    first_string(row, &fields, &["entity_id", "id", "Id"], &["Id", "id"])
}
