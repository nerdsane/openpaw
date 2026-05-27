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
    post_directed_evolution_action_with_headers(
        client,
        config,
        &request_headers,
        "Episodes",
        &episode_id,
        "BeginEpisodeNegotiation",
        json!({
            "DirectionId": plan.direction_id,
            "OrganismId": plan.organism_id,
            "ParentVersionId": plan.parent_version_id,
            "AutonomyLane": plan.autonomy_lane,
        }),
    )
    .await?;

    let metric_ids = activate_directed_evolution_human_metrics(
        client,
        config,
        &request_headers,
        &plan.metrics,
    )
    .await?;
    let metric_ids_by_name = plan
        .metrics
        .iter()
        .map(|metric| metric.name.clone())
        .zip(metric_ids.iter().cloned())
        .collect::<std::collections::BTreeMap<_, _>>();
    let adaptation_goal_id = activate_directed_evolution_human_goal(
        client,
        config,
        &request_headers,
        &episode_id,
        &plan,
    )
    .await?;
    let viability_constraint_ids = activate_directed_evolution_human_constraints(
        client,
        config,
        &request_headers,
        &episode_id,
        &plan,
    )
    .await?;
    let elimination_rule_ids = activate_directed_evolution_human_elimination_rules(
        client,
        config,
        &request_headers,
        &episode_id,
        &plan,
        &metric_ids,
        &metric_ids_by_name,
    )
    .await?;
    let scoring_rule_ids = activate_directed_evolution_human_scoring_rules(
        client,
        config,
        &request_headers,
        &episode_id,
        &plan,
        &metric_ids,
        &metric_ids_by_name,
    )
    .await?;
    let selection_pressure_id = activate_directed_evolution_human_selection_pressure(
        client,
        config,
        &request_headers,
        &episode_id,
        &plan,
        DirectedEvolutionSelectionInputs {
            metric_ids: &metric_ids,
            elimination_rule_ids: &elimination_rule_ids,
            scoring_rule_ids: &scoring_rule_ids,
        },
    )
    .await?;
    let evaluation_stage_ids = activate_directed_evolution_human_evaluation_stages(
        client,
        config,
        &request_headers,
        &episode_id,
        &plan,
    )
    .await?;

    post_directed_evolution_action_with_headers(
        client,
        config,
        &request_headers,
        "Episodes",
        &episode_id,
        "RecordEpisodeContract",
        json!({
            "AdaptationGoalId": adaptation_goal_id,
            "SelectionPressureId": selection_pressure_id,
            "ViabilityConstraintIdsJson": json!(viability_constraint_ids).to_string(),
            "EvaluationStageIdsJson": json!(evaluation_stage_ids).to_string(),
            "EliminationRuleIdsJson": json!(elimination_rule_ids).to_string(),
            "ScoringRuleIdsJson": json!(scoring_rule_ids).to_string(),
        }),
    )
    .await?;
    post_directed_evolution_action_with_headers(
        client,
        config,
        &request_headers,
        "Directions",
        &plan.direction_id,
        "SelectDirection",
        json!({
            "EpisodeId": episode_id,
            "SelectedBy": plan.selected_by,
            "SelectionNotes": plan.selection_notes,
        }),
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
        "selection_pressure_id": selection_pressure_id,
        "metric_definition_ids": metric_ids,
        "viability_constraint_ids": viability_constraint_ids,
        "elimination_rule_ids": elimination_rule_ids,
        "scoring_rule_ids": scoring_rule_ids,
        "evaluation_stage_ids": evaluation_stage_ids,
    }))
}
