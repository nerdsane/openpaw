async fn run_directed_evolution_human_episode_command(
    client: &reqwest::Client,
    config: &Config,
    contract_path: Option<&str>,
) -> Result<Value> {
    let input = read_directed_evolution_human_episode_input(contract_path)?;
    start_directed_evolution_human_episode(client, config, input).await
}

fn read_directed_evolution_human_episode_input(
    contract_path: Option<&str>,
) -> Result<DirectedEvolutionHumanEpisodeInput> {
    let raw = if let Some(path) = contract_path.filter(|value| !value.trim().is_empty()) {
        fs::read_to_string(path).with_context(|| format!("read Directed Evolution contract {path}"))?
    } else {
        let mut raw = String::new();
        io::stdin()
            .read_to_string(&mut raw)
            .context("read Directed Evolution contract from stdin")?;
        raw
    };
    serde_json::from_str(&raw).context("parse Directed Evolution episode contract JSON")
}

async fn start_directed_evolution_human_episode(
    client: &reqwest::Client,
    config: &Config,
    input: DirectedEvolutionHumanEpisodeInput,
) -> Result<Value> {
    let request_headers = directed_evolution_director_headers(config)?;
    let direction_fields = fetch_directed_evolution_entity_fields_with_headers(
        client,
        config,
        &request_headers,
        "Directions",
        &input.direction_id,
    )
    .await?;
    let organism_id = nonempty(
        input.organism_id.clone(),
        value_field_string(&direction_fields, &["OrganismId", "organism_id"]),
    );
    if organism_id.trim().is_empty() {
        bail!("Directed Evolution episode contract must provide organism_id or target a Direction with OrganismId");
    }
    let organism_fields = fetch_directed_evolution_entity_fields_with_headers(
        client,
        config,
        &request_headers,
        "Organisms",
        &organism_id,
    )
    .await?;
    let plan =
        directed_evolution_episode_plan_from_input(input, &direction_fields, &organism_fields)?;

    let episode_id = create_directed_evolution_entity_with_headers(
        client,
        config,
        &request_headers,
        "Episodes",
    )
    .await?;
    let adaptation_goal_id = create_directed_evolution_entity_with_headers(
        client,
        config,
        &request_headers,
        "AdaptationGoals",
    )
    .await?;
    post_directed_evolution_action_with_headers(
        client,
        config,
        &request_headers,
        "AdaptationGoals",
        &adaptation_goal_id,
        "ActivateAdaptationGoal",
        json!({
            "EpisodeId": episode_id,
            "GoalStatement": plan.adaptation_goal,
            "CreatedByBrainRunId": plan.created_by_brain_run_id,
            "HumanNotes": plan.human_notes,
        }),
    )
    .await?;

    let mut metric_ids = Vec::new();
    let mut metric_name_to_id = std::collections::BTreeMap::new();
    for metric in &plan.metrics {
        let metric_id = create_directed_evolution_entity_with_headers(
            client,
            config,
            &request_headers,
            "MetricDefinitions",
        )
        .await?;
        post_directed_evolution_action_with_headers(
            client,
            config,
            &request_headers,
            "MetricDefinitions",
            &metric_id,
            "ActivateMetricDefinitionWithProvenance",
            json!({
                "EpisodeId": episode_id,
                "MetricName": metric.name,
                "MetricKind": metric.kind,
                "Unit": metric.unit,
                "HigherIsBetter": metric.higher_is_better.to_string(),
                "Description": metric.description,
                "ProvenanceKind": metric.provenance_kind,
                "EvaluatorRef": metric.evaluator_ref,
                "EvaluatorModule": metric.evaluator_module,
                "Interpretation": metric.interpretation,
                "HardConstraint": metric.hard_constraint.to_string(),
            }),
        )
        .await?;
        metric_name_to_id.insert(metric.name.clone(), metric_id.clone());
        metric_ids.push(metric_id);
    }

    let mut viability_constraint_ids = Vec::new();
    for constraint in &plan.viability_constraints {
        let constraint_id = create_directed_evolution_entity_with_headers(
            client,
            config,
            &request_headers,
            "ViabilityConstraints",
        )
        .await?;
        post_directed_evolution_action_with_headers(
            client,
            config,
            &request_headers,
            "ViabilityConstraints",
            &constraint_id,
            "ActivateViabilityConstraint",
            json!({
                "EpisodeId": episode_id,
                "ConstraintStatement": constraint.statement,
                "ConstraintKind": constraint.kind,
                "CreatedByBrainRunId": plan.created_by_brain_run_id,
            }),
        )
        .await?;
        viability_constraint_ids.push(constraint_id);
    }

    let mut evaluation_stage_ids = Vec::new();
    for (index, stage) in plan.evaluation_stages.iter().enumerate() {
        let stage_id = create_directed_evolution_entity_with_headers(
            client,
            config,
            &request_headers,
            "EvaluationStages",
        )
        .await?;
        post_directed_evolution_action_with_headers(
            client,
            config,
            &request_headers,
            "EvaluationStages",
            &stage_id,
            "ActivateEvaluationStageWithEvaluator",
            json!({
                "EpisodeId": episode_id,
                "StageName": stage.name,
                "StageKind": stage.kind,
                "SequenceIndex": (index + 1).to_string(),
                "RequiredEvidenceJson": json!(stage.required_evidence).to_string(),
                "ExecutorKind": stage.executor,
                "MeasurementProvenance": stage.measurement_provenance,
                "EvaluatorRef": stage.evaluator_ref,
                "EvaluatorModule": stage.evaluator_module,
                "DecisionAuthority": stage.decision_authority,
            }),
        )
        .await?;
        evaluation_stage_ids.push(stage_id);
    }

    let mut elimination_rule_ids = Vec::new();
    for rule in &plan.elimination_rules {
        let metric_ids_for_rule = rule_metric_ids(rule.metric_ids.clone(), &rule.metric_names, &metric_name_to_id);
        let rule_id = create_directed_evolution_entity_with_headers(
            client,
            config,
            &request_headers,
            "EliminationRules",
        )
        .await?;
        post_directed_evolution_action_with_headers(
            client,
            config,
            &request_headers,
            "EliminationRules",
            &rule_id,
            "ActivateEliminationRule",
            json!({
                "EpisodeId": episode_id,
                "RuleStatement": rule.statement,
                "MetricIdsJson": json!(metric_ids_for_rule).to_string(),
                "ThresholdJson": rule.threshold.to_string(),
                "CreatedByBrainRunId": plan.created_by_brain_run_id,
            }),
        )
        .await?;
        elimination_rule_ids.push(rule_id);
    }

    let mut scoring_rule_ids = Vec::new();
    for rule in &plan.scoring_rules {
        let metric_ids_for_rule = rule_metric_ids(rule.metric_ids.clone(), &rule.metric_names, &metric_name_to_id);
        let rule_id = create_directed_evolution_entity_with_headers(
            client,
            config,
            &request_headers,
            "ScoringRules",
        )
        .await?;
        post_directed_evolution_action_with_headers(
            client,
            config,
            &request_headers,
            "ScoringRules",
            &rule_id,
            "ActivateScoringRule",
            json!({
                "EpisodeId": episode_id,
                "RuleStatement": rule.statement,
                "MetricIdsJson": json!(metric_ids_for_rule).to_string(),
                "Weight": rule.weight,
                "CreatedByBrainRunId": plan.created_by_brain_run_id,
            }),
        )
        .await?;
        scoring_rule_ids.push(rule_id);
    }

    let simulated_user_plan_id = create_directed_evolution_entity_with_headers(
        client,
        config,
        &request_headers,
        "SimulatedUserPlans",
    )
    .await?;
    post_directed_evolution_action_with_headers(
        client,
        config,
        &request_headers,
        "SimulatedUserPlans",
        &simulated_user_plan_id,
        "ActivateSimulatedUserPlan",
        json!({
            "EpisodeId": episode_id,
            "UsersPerVariant": plan.simulated_user_plan.users_per_variant.to_string(),
            "RunsPerPersona": plan.simulated_user_plan.runs_per_persona.to_string(),
            "PersonasJson": json!(plan.simulated_user_plan.personas).to_string(),
            "GoalsJson": json!(plan.simulated_user_plan.goals).to_string(),
            "CreatedBy": plan.selected_by,
            "HumanDecisionSummary": plan.simulated_user_plan.human_decision_summary,
        }),
    )
    .await?;
    post_directed_evolution_action_with_headers(
        client,
        config,
        &request_headers,
        "SimulatedUserPlans",
        &simulated_user_plan_id,
        "FreezeSimulatedUserPlan",
        json!({
            "FrozenBy": plan.selected_by,
            "Reason": "Frozen before Episode.Start so simulated-user count cannot drift mid-generation.",
        }),
    )
    .await?;

    let selection_protocol_id = create_directed_evolution_entity_with_headers(
        client,
        config,
        &request_headers,
        "SelectionProtocols",
    )
    .await?;
    post_directed_evolution_action_with_headers(
        client,
        config,
        &request_headers,
        "SelectionProtocols",
        &selection_protocol_id,
        "ActivateSelectionProtocol",
        json!({
            "EpisodeId": episode_id,
            "SelectionStatement": plan.selection_statement,
            "MetricIdsJson": json!(metric_ids).to_string(),
            "EliminationRuleIdsJson": json!(elimination_rule_ids).to_string(),
            "ScoringRuleIdsJson": json!(scoring_rule_ids).to_string(),
            "EvaluatorRef": plan.evaluator_ref,
            "DecisionPolicy": plan.selection_notes,
            "CreatedByBrainRunId": plan.created_by_brain_run_id,
        }),
    )
    .await?;
    post_directed_evolution_action_with_headers(
        client,
        config,
        &request_headers,
        "SelectionProtocols",
        &selection_protocol_id,
        "FreezeSelectionProtocol",
        json!({
            "FrozenBy": plan.selected_by,
            "Reason": "Frozen before Episode.Start so variants cannot move the goalposts.",
        }),
    )
    .await?;

    post_directed_evolution_action_with_headers(
        client,
        config,
        &request_headers,
        "Episodes",
        &episode_id,
        "PlanEpisode",
        json!({
            "DirectionId": plan.direction_id,
            "OrganismId": plan.organism_id,
            "ParentVersionId": plan.parent_version_id,
            "AutonomyLane": plan.autonomy_lane,
            "AdaptationGoalId": adaptation_goal_id,
            "ViabilityConstraintIdsJson": json!(viability_constraint_ids).to_string(),
            "MetricDefinitionIdsJson": json!(metric_ids).to_string(),
            "EvaluationStageIdsJson": json!(evaluation_stage_ids).to_string(),
            "EliminationRuleIdsJson": json!(elimination_rule_ids).to_string(),
            "ScoringRuleIdsJson": json!(scoring_rule_ids).to_string(),
            "SimulatedUserPlanId": simulated_user_plan_id,
            "SelectionProtocolId": selection_protocol_id,
            "EvaluatorRef": plan.evaluator_ref,
            "OrganismParentRef": plan.parent_version_id,
            "PlannedBy": plan.selected_by,
            "PlanSummary": plan.human_notes,
        }),
    )
    .await?;
    select_directed_evolution_direction_for_episode(
        client,
        config,
        &request_headers,
        &plan.direction_id,
        &episode_id,
        &plan.selected_by,
        &plan.selection_notes,
    )
    .await?;
    post_directed_evolution_action_with_headers(
        client,
        config,
        &request_headers,
        "Episodes",
        &episode_id,
        "StartEpisode",
        json!({
            "StartedBy": plan.started_by,
            "Reason": plan.start_reason,
        }),
    )
    .await?;

    Ok(json!({
        "status": "started",
        "episode_id": episode_id,
        "direction_id": plan.direction_id,
        "organism_id": plan.organism_id,
        "parent_version_id": plan.parent_version_id,
        "autonomy_lane": plan.autonomy_lane,
        "adaptation_goal_id": adaptation_goal_id,
        "metric_definition_ids": metric_ids,
        "viability_constraint_ids": viability_constraint_ids,
        "evaluation_stage_ids": evaluation_stage_ids,
        "elimination_rule_ids": elimination_rule_ids,
        "scoring_rule_ids": scoring_rule_ids,
        "simulated_user_plan_id": simulated_user_plan_id,
        "selection_protocol_id": selection_protocol_id,
        "semantic_temper_entities": true,
    }))
}

async fn select_directed_evolution_direction_for_episode(
    client: &reqwest::Client,
    config: &Config,
    request_headers: &HeaderMap,
    direction_id: &str,
    episode_id: &str,
    selected_by: &str,
    selection_notes: &str,
) -> Result<()> {
    let direction = fetch_directed_evolution_entity_with_headers(
        client,
        config,
        request_headers,
        "Directions",
        direction_id,
    )
    .await?;
    let direction_fields = direction.get("fields").cloned().unwrap_or_else(|| json!({}));
    let status = first_string(
        &direction,
        &direction_fields,
        &["status", "Status"],
        &["status", "Status"],
    );
    if status == "Selected" {
        return Ok(());
    }
    post_directed_evolution_action_with_headers(
        client,
        config,
        request_headers,
        "Directions",
        direction_id,
        "SelectDirection",
        json!({
            "EpisodeId": episode_id,
            "SelectedBy": selected_by,
            "SelectionNotes": selection_notes,
        }),
    )
    .await
}

fn rule_metric_ids(
    explicit_metric_ids: Vec<String>,
    metric_names: &[String],
    metric_name_to_id: &std::collections::BTreeMap<String, String>,
) -> Vec<String> {
    let mut ids = explicit_metric_ids
        .into_iter()
        .filter(|id| !id.trim().is_empty())
        .collect::<Vec<_>>();
    for name in metric_names {
        if let Some(metric_id) = metric_name_to_id.get(name) {
            ids.push(metric_id.clone());
        }
    }
    ids
}

async fn create_directed_evolution_entity_with_headers(
    client: &reqwest::Client,
    config: &Config,
    request_headers: &HeaderMap,
    entity_set: &str,
) -> Result<String> {
    let response = client
        .post(format!("{}/tdata/{}", config.temper_url, entity_set))
        .headers(request_headers.clone())
        .json(&json!({}))
        .send()
        .await
        .with_context(|| format!("create Directed Evolution {entity_set}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        bail!("create Directed Evolution {entity_set} returned {status}: {text}");
    }
    let value: Value = response
        .json()
        .await
        .context("parse Directed Evolution create response")?;
    let fields = value.get("fields").cloned().unwrap_or_else(|| json!({}));
    let id = first_string(&value, &fields, &["entity_id", "id", "Id"], &["id", "Id"]);
    if id.is_empty() {
        bail!("create Directed Evolution {entity_set} response was missing entity_id");
    }
    Ok(id)
}

async fn fetch_directed_evolution_entity_fields_with_headers(
    client: &reqwest::Client,
    config: &Config,
    request_headers: &HeaderMap,
    entity_set: &str,
    entity_id: &str,
) -> Result<Value> {
    let response = client
        .get(config.entity_url(entity_set, entity_id))
        .headers(request_headers.clone())
        .send()
        .await
        .with_context(|| format!("fetch Directed Evolution {entity_set}('{entity_id}')"))?;
    if !response.status().is_success() {
        bail!(
            "fetch Directed Evolution {}('{}') returned {}",
            entity_set,
            entity_id,
            response.status()
        );
    }
    let body: Value = response
        .json()
        .await
        .context("parse Directed Evolution entity")?;
    Ok(body.get("fields").cloned().unwrap_or_else(|| json!({})))
}

async fn fetch_directed_evolution_entity_with_headers(
    client: &reqwest::Client,
    config: &Config,
    request_headers: &HeaderMap,
    entity_set: &str,
    entity_id: &str,
) -> Result<Value> {
    let response = client
        .get(config.entity_url(entity_set, entity_id))
        .headers(request_headers.clone())
        .send()
        .await
        .with_context(|| format!("fetch Directed Evolution {entity_set}('{entity_id}')"))?;
    if !response.status().is_success() {
        bail!(
            "fetch Directed Evolution {}('{}') returned {}",
            entity_set,
            entity_id,
            response.status()
        );
    }
    response
        .json()
        .await
        .context("parse Directed Evolution entity")
}

async fn post_directed_evolution_action_with_headers(
    client: &reqwest::Client,
    config: &Config,
    request_headers: &HeaderMap,
    entity_set: &str,
    entity_id: &str,
    action: &str,
    body: Value,
) -> Result<()> {
    let url = config.entity_action_url_with_namespace(
        entity_set,
        entity_id,
        DIRECTED_EVOLUTION_NAMESPACE,
        action,
    );
    let response = client
        .post(url)
        .headers(request_headers.clone())
        .json(&body)
        .send()
        .await
        .with_context(|| format!("dispatch Directed Evolution {entity_set}.{action}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        bail!("Directed Evolution {entity_set}.{action} returned {status}: {text}");
    }
    Ok(())
}

fn directed_evolution_director_headers(config: &Config) -> Result<HeaderMap> {
    let mut request_headers = headers(config)?;
    let principal_id =
        env::var("DIRECTED_EVOLUTION_DIRECTOR_ID").unwrap_or_else(|_| config.worker_id.clone());
    let agent_type =
        env::var("DIRECTED_EVOLUTION_DIRECTOR_AGENT_TYPE").unwrap_or_else(|_| "codex".to_string());
    request_headers.insert(
        "x-temper-principal-id",
        HeaderValue::from_str(&principal_id).context("invalid DIRECTED_EVOLUTION_DIRECTOR_ID")?,
    );
    request_headers.insert("x-temper-principal-kind", HeaderValue::from_static("agent"));
    request_headers.insert(
        "x-temper-agent-type",
        HeaderValue::from_str(&agent_type)
            .context("invalid DIRECTED_EVOLUTION_DIRECTOR_AGENT_TYPE")?,
    );
    request_headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    request_headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    Ok(request_headers)
}
