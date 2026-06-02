async fn post_directed_evolution_action(
    client: &reqwest::Client,
    config: &Config,
    entity_set: &str,
    entity_id: &str,
    action: &str,
    body: Value,
) -> Result<()> {
    post_entity_action_with_namespace(
        client,
        config,
        entity_set,
        entity_id,
        DIRECTED_EVOLUTION_NAMESPACE,
        action,
        body,
    )
    .await
}

async fn post_paw_orchestration_action(
    client: &reqwest::Client,
    config: &Config,
    entity_set: &str,
    entity_id: &str,
    action: &str,
    body: Value,
) -> Result<()> {
    post_entity_action_with_namespace(
        client,
        config,
        entity_set,
        entity_id,
        PAW_ORCHESTRATION_NAMESPACE,
        action,
        body,
    )
    .await
}

async fn post_paw_orchestration_action_with_headers(
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
        PAW_ORCHESTRATION_NAMESPACE,
        action,
    );
    let response = client
        .post(url)
        .headers(request_headers.clone())
        .json(&body)
        .send()
        .await
        .with_context(|| format!("dispatch PawOrchestration {entity_set}.{action}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        bail!("PawOrchestration {entity_set}.{action} returned {status}: {text}");
    }
    Ok(())
}

async fn route_directed_evolution_work_item_success(
    client: &reqwest::Client,
    config: &Config,
    work_item: &DirectedEvolutionWorkItemState,
    worker_run_id: &str,
    output_json: &str,
    evidence_artifact_id: &str,
    summary: &str,
) -> Result<()> {
    let receipt_id = create_entity(client, config, "WorkItemReceipts", json!({})).await?;
    post_directed_evolution_action(
        client,
        config,
        "WorkItemReceipts",
        &receipt_id,
        "RouteSucceededWorkItem",
        json!({
            "WorkItemId": work_item.id,
            "Role": work_item.role,
            "TargetEntityType": work_item.target_entity_type,
            "TargetEntityId": work_item.target_entity_id,
            "WorkerRunId": worker_run_id,
            "ResultJson": output_json,
            "EvidenceArtifactId": evidence_artifact_id,
            "Summary": summary,
            "CorrelationJson": work_item.correlation_json,
        }),
    )
    .await
}

async fn route_directed_evolution_work_item_failure(
    client: &reqwest::Client,
    config: &Config,
    work_item: &DirectedEvolutionWorkItemState,
    worker_run_id: &str,
    failure_reason: &str,
    evidence_artifact_id: &str,
) -> Result<()> {
    let receipt_id = create_entity(client, config, "WorkItemReceipts", json!({})).await?;
    post_directed_evolution_action(
        client,
        config,
        "WorkItemReceipts",
        &receipt_id,
        "RouteFailedWorkItem",
        json!({
            "WorkItemId": work_item.id,
            "Role": work_item.role,
            "TargetEntityType": work_item.target_entity_type,
            "TargetEntityId": work_item.target_entity_id,
            "WorkerRunId": worker_run_id,
            "FailureReason": failure_reason,
            "EvidenceArtifactId": evidence_artifact_id,
            "CorrelationJson": work_item.correlation_json,
        }),
    )
    .await
}
