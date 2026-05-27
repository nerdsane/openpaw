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

    let request_id = create_directed_evolution_entity_with_headers(
        client,
        config,
        &request_headers,
        "EpisodeStartRequests",
    )
    .await?;
    post_directed_evolution_action_with_headers(
        client,
        config,
        &request_headers,
        "EpisodeStartRequests",
        &request_id,
        "SubmitEpisodeStartRequest",
        directed_evolution_episode_start_request_body(&plan),
    )
    .await?;
    let request_fields = fetch_directed_evolution_entity_fields_with_headers(
        client,
        config,
        &request_headers,
        "EpisodeStartRequests",
        &request_id,
    )
    .await
    .unwrap_or_else(|_| json!({}));
    let episode_id = value_field_string(&request_fields, &["EpisodeId", "episode_id"]);

    Ok(json!({
        "status": if episode_id.trim().is_empty() { "submitted" } else { "started" },
        "episode_start_request_id": request_id,
        "episode_id": episode_id,
        "direction_id": plan.direction_id,
        "organism_id": plan.organism_id,
        "parent_version_id": plan.parent_version_id,
        "autonomy_lane": plan.autonomy_lane,
        "app_owned_materialization": true,
    }))
}

fn directed_evolution_episode_start_request_body(plan: &DirectedEvolutionEpisodePlan) -> Value {
    json!({
        "DirectionId": plan.direction_id,
        "OrganismId": plan.organism_id,
        "ParentVersionId": plan.parent_version_id,
        "AutonomyLane": plan.autonomy_lane,
        "RequestedBy": plan.created_by_brain_run_id,
        "AdaptationGoal": plan.adaptation_goal,
        "HumanNotes": plan.human_notes,
        "ViabilityConstraintsJson": json!(plan.viability_constraints.iter().map(|constraint| {
            json!({
                "statement": constraint.statement,
                "kind": constraint.kind,
            })
        }).collect::<Vec<_>>()).to_string(),
        "MetricsJson": json!(plan.metrics.iter().map(|metric| {
            json!({
                "name": metric.name,
                "kind": metric.kind,
                "unit": metric.unit,
                "higher_is_better": metric.higher_is_better.to_string(),
                "description": metric.description,
            })
        }).collect::<Vec<_>>()).to_string(),
        "EvaluationStagesJson": json!(plan.evaluation_stages.iter().map(|stage| {
            json!({
                "name": stage.name,
                "kind": stage.kind,
                "executor": stage.executor,
                "required_evidence": stage.required_evidence,
            })
        }).collect::<Vec<_>>()).to_string(),
        "EliminationRulesJson": json!(plan.elimination_rules.iter().map(|rule| {
            json!({
                "statement": rule.statement,
                "metric_names": rule.metric_names,
                "metric_ids": rule.metric_ids,
                "threshold": rule.threshold,
            })
        }).collect::<Vec<_>>()).to_string(),
        "ScoringRulesJson": json!(plan.scoring_rules.iter().map(|rule| {
            json!({
                "statement": rule.statement,
                "metric_names": rule.metric_names,
                "metric_ids": rule.metric_ids,
                "weight": rule.weight,
            })
        }).collect::<Vec<_>>()).to_string(),
        "SelectionStatement": plan.selection_statement,
        "ContractJson": json!({
            "contract_version": "directed-evolution.episode-start-request.v1",
            "selected_by": plan.selected_by,
            "selection_notes": plan.selection_notes,
            "source": "temperpaw-codex-worker",
        }).to_string(),
        "StartedBy": plan.started_by,
        "Reason": plan.start_reason,
    })
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

async fn post_directed_evolution_action_with_headers(
    client: &reqwest::Client,
    config: &Config,
    request_headers: &HeaderMap,
    entity_set: &str,
    entity_id: &str,
    action: &str,
    body: Value,
) -> Result<()> {
    let mut url = config.entity_action_url_with_namespace(
        entity_set,
        entity_id,
        DIRECTED_EVOLUTION_NAMESPACE,
        action,
    );
    if action == "SubmitEpisodeStartRequest" {
        url.push_str("?await_integration=true");
    }
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
