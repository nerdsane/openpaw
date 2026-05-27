async fn activate_directed_evolution_human_metrics(
    client: &reqwest::Client,
    config: &Config,
    request_headers: &HeaderMap,
    metrics: &[DirectedEvolutionMetricPlan],
) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    for metric in metrics {
        let metric_id = create_directed_evolution_entity_with_headers(
            client,
            config,
            request_headers,
            "MetricDefinitions",
        )
        .await?;
        post_directed_evolution_action_with_headers(
            client,
            config,
            request_headers,
            "MetricDefinitions",
            &metric_id,
            "ActivateMetricDefinition",
            json!({
                "MetricName": metric.name,
                "MetricKind": metric.kind,
                "Unit": metric.unit,
                "HigherIsBetter": metric.higher_is_better.to_string(),
                "Description": metric.description,
            }),
        )
        .await?;
        ids.push(metric_id);
    }
    Ok(ids)
}

async fn activate_directed_evolution_human_goal(
    client: &reqwest::Client,
    config: &Config,
    request_headers: &HeaderMap,
    episode_id: &str,
    plan: &DirectedEvolutionEpisodePlan,
) -> Result<String> {
    let goal_id = create_directed_evolution_entity_with_headers(
        client,
        config,
        request_headers,
        "AdaptationGoals",
    )
    .await?;
    post_directed_evolution_action_with_headers(
        client,
        config,
        request_headers,
        "AdaptationGoals",
        &goal_id,
        "ActivateAdaptationGoal",
        json!({
            "EpisodeId": episode_id,
            "GoalStatement": plan.adaptation_goal,
            "CreatedByBrainRunId": plan.created_by_brain_run_id,
            "HumanNotes": plan.human_notes,
        }),
    )
    .await?;
    Ok(goal_id)
}

async fn activate_directed_evolution_human_constraints(
    client: &reqwest::Client,
    config: &Config,
    request_headers: &HeaderMap,
    episode_id: &str,
    plan: &DirectedEvolutionEpisodePlan,
) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    for constraint in &plan.viability_constraints {
        let constraint_id = create_directed_evolution_entity_with_headers(
            client,
            config,
            request_headers,
            "ViabilityConstraints",
        )
        .await?;
        post_directed_evolution_action_with_headers(
            client,
            config,
            request_headers,
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
        ids.push(constraint_id);
    }
    Ok(ids)
}

async fn activate_directed_evolution_human_elimination_rules(
    client: &reqwest::Client,
    config: &Config,
    request_headers: &HeaderMap,
    episode_id: &str,
    plan: &DirectedEvolutionEpisodePlan,
    all_metric_ids: &[String],
    metric_ids_by_name: &std::collections::BTreeMap<String, String>,
) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    for rule in &plan.elimination_rules {
        let rule_id = create_directed_evolution_entity_with_headers(
            client,
            config,
            request_headers,
            "EliminationRules",
        )
        .await?;
        post_directed_evolution_action_with_headers(
            client,
            config,
            request_headers,
            "EliminationRules",
            &rule_id,
            "ActivateEliminationRule",
            json!({
                "EpisodeId": episode_id,
                "RuleStatement": rule.statement,
                "MetricIdsJson": json!(metric_ids_for_rule(&rule.metric_ids, &rule.metric_names, all_metric_ids, metric_ids_by_name)).to_string(),
                "ThresholdJson": rule.threshold.to_string(),
                "CreatedByBrainRunId": plan.created_by_brain_run_id,
            }),
        )
        .await?;
        ids.push(rule_id);
    }
    Ok(ids)
}

async fn activate_directed_evolution_human_scoring_rules(
    client: &reqwest::Client,
    config: &Config,
    request_headers: &HeaderMap,
    episode_id: &str,
    plan: &DirectedEvolutionEpisodePlan,
    all_metric_ids: &[String],
    metric_ids_by_name: &std::collections::BTreeMap<String, String>,
) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    for rule in &plan.scoring_rules {
        let rule_id = create_directed_evolution_entity_with_headers(
            client,
            config,
            request_headers,
            "ScoringRules",
        )
        .await?;
        post_directed_evolution_action_with_headers(
            client,
            config,
            request_headers,
            "ScoringRules",
            &rule_id,
            "ActivateScoringRule",
            json!({
                "EpisodeId": episode_id,
                "RuleStatement": rule.statement,
                "MetricIdsJson": json!(metric_ids_for_rule(&rule.metric_ids, &rule.metric_names, all_metric_ids, metric_ids_by_name)).to_string(),
                "Weight": rule.weight,
                "CreatedByBrainRunId": plan.created_by_brain_run_id,
            }),
        )
        .await?;
        ids.push(rule_id);
    }
    Ok(ids)
}

async fn activate_directed_evolution_human_selection_pressure(
    client: &reqwest::Client,
    config: &Config,
    request_headers: &HeaderMap,
    episode_id: &str,
    plan: &DirectedEvolutionEpisodePlan,
    inputs: DirectedEvolutionSelectionInputs<'_>,
) -> Result<String> {
    let selection_pressure_id = create_directed_evolution_entity_with_headers(
        client,
        config,
        request_headers,
        "SelectionPressures",
    )
    .await?;
    post_directed_evolution_action_with_headers(
        client,
        config,
        request_headers,
        "SelectionPressures",
        &selection_pressure_id,
        "ActivateSelectionPressure",
        json!({
            "EpisodeId": episode_id,
            "SelectionStatement": plan.selection_statement,
            "MetricIdsJson": json!(inputs.metric_ids).to_string(),
            "EliminationRuleIdsJson": json!(inputs.elimination_rule_ids).to_string(),
            "ScoringRuleIdsJson": json!(inputs.scoring_rule_ids).to_string(),
            "CreatedByBrainRunId": plan.created_by_brain_run_id,
        }),
    )
    .await?;
    Ok(selection_pressure_id)
}

async fn activate_directed_evolution_human_evaluation_stages(
    client: &reqwest::Client,
    config: &Config,
    request_headers: &HeaderMap,
    episode_id: &str,
    plan: &DirectedEvolutionEpisodePlan,
) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    for (index, stage) in plan.evaluation_stages.iter().enumerate() {
        let stage_id = create_directed_evolution_entity_with_headers(
            client,
            config,
            request_headers,
            "EvaluationStages",
        )
        .await?;
        post_directed_evolution_action_with_headers(
            client,
            config,
            request_headers,
            "EvaluationStages",
            &stage_id,
            "ActivateEvaluationStage",
            json!({
                "EpisodeId": episode_id,
                "StageName": stage.name,
                "StageKind": stage.kind,
                "SequenceIndex": index + 1,
                "RequiredEvidenceJson": json!(stage.required_evidence).to_string(),
                "ExecutorKind": stage.executor,
            }),
        )
        .await?;
        ids.push(stage_id);
    }
    Ok(ids)
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
    let body: Value = response.json().await.context("parse Directed Evolution entity")?;
    Ok(body.get("fields").cloned().unwrap_or_else(|| json!({})))
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
    let response = client
        .post(config.entity_action_url_with_namespace(
            entity_set,
            entity_id,
            DIRECTED_EVOLUTION_NAMESPACE,
            action,
        ))
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
