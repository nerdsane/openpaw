fn directed_evolution_claim_conflict_message(message: &str) -> bool {
    message.contains("WorkItems.ClaimWorkItem returned 409")
        && message.contains("not valid from state")
}

struct DirectedEvolutionExclusiveKeyConflict {
    exclusive_key: String,
    active_work_item_id: String,
}

async fn directed_evolution_exclusive_key_conflict(
    client: &reqwest::Client,
    config: &Config,
    work_item: &DirectedEvolutionWorkItemState,
) -> Result<Option<DirectedEvolutionExclusiveKeyConflict>> {
    directed_evolution_exclusive_key_conflict_with_policy(
        client,
        config,
        work_item,
        ExclusiveKeyConflictPolicy::AnyActivePeer,
    )
    .await
}

async fn directed_evolution_exclusive_key_post_claim_conflict(
    client: &reqwest::Client,
    config: &Config,
    work_item: &DirectedEvolutionWorkItemState,
) -> Result<Option<DirectedEvolutionExclusiveKeyConflict>> {
    directed_evolution_exclusive_key_conflict_with_policy(
        client,
        config,
        work_item,
        ExclusiveKeyConflictPolicy::LowerActivePeer,
    )
    .await
}

#[derive(Clone, Copy)]
enum ExclusiveKeyConflictPolicy {
    AnyActivePeer,
    LowerActivePeer,
}

async fn directed_evolution_exclusive_key_conflict_with_policy(
    client: &reqwest::Client,
    config: &Config,
    work_item: &DirectedEvolutionWorkItemState,
    policy: ExclusiveKeyConflictPolicy,
) -> Result<Option<DirectedEvolutionExclusiveKeyConflict>> {
    let fields =
        fetch_directed_evolution_entity_fields(client, config, "WorkItems", &work_item.id).await?;
    let exclusive_key = value_field_string(&fields, &["ExclusiveKey", "exclusive_key"]);
    if exclusive_key.trim().is_empty() {
        return Ok(None);
    }

    let escaped_key = exclusive_key.replace('\'', "''");
    let rows = query_directed_evolution_rows(
        client,
        config,
        "WorkItems",
        &format!("ExclusiveKey eq '{escaped_key}'"),
        100,
    )
    .await?;
    if let Some(active_work_item_id) =
        directed_evolution_exclusive_key_conflict_from_rows(&work_item.id, &rows, policy)
    {
        return Ok(Some(DirectedEvolutionExclusiveKeyConflict {
            exclusive_key,
            active_work_item_id,
        }));
    }

    Ok(None)
}

fn directed_evolution_exclusive_key_conflict_from_rows(
    work_item_id: &str,
    rows: &[Value],
    policy: ExclusiveKeyConflictPolicy,
) -> Option<String> {
    let mut active_ids = Vec::new();
    for row in rows {
        let row_fields = directed_evolution_row_fields(row);
        let row_id = directed_evolution_row_id(row);
        if row_id == work_item_id {
            continue;
        }
        let status = value_field_string(&row_fields, &["Status", "status"]);
        if matches!(status.as_str(), "Claimed" | "Running") {
            active_ids.push(row_id);
        }
    }
    active_ids.sort();
    match policy {
        ExclusiveKeyConflictPolicy::AnyActivePeer => active_ids.into_iter().next(),
        ExclusiveKeyConflictPolicy::LowerActivePeer => active_ids
            .into_iter()
            .find(|active_id| active_id.as_str() < work_item_id),
    }
}

async fn stale_directed_evolution_work_item_reason(
    client: &reqwest::Client,
    config: &Config,
    work_item: &DirectedEvolutionWorkItemState,
) -> Result<Option<String>> {
    if !directed_evolution_stage_evaluator_role(&work_item.role)
        || work_item.target_entity_type != "StageResult"
    {
        return Ok(None);
    }

    let stage_fields = fetch_directed_evolution_entity_fields(
        client,
        config,
        "StageResults",
        &work_item.target_entity_id,
    )
    .await?;
    let variant_fields = {
        let variant_id = value_field_string(&stage_fields, &["VariantId", "variant_id"]);
        if variant_id.trim().is_empty() {
            json!({})
        } else {
            fetch_directed_evolution_entity_fields(client, config, "Variants", &variant_id).await?
        }
    };
    Ok(stale_directed_evolution_stage_work_reason(
        work_item,
        &stage_fields,
        &variant_fields,
    ))
}

async fn eliminate_stale_directed_evolution_stage_result(
    client: &reqwest::Client,
    config: &Config,
    work_item: &DirectedEvolutionWorkItemState,
    reason: &str,
) -> Result<()> {
    if !stale_stage_work_targets_stage_result(work_item) {
        return Ok(());
    }

    let stage_fields = fetch_directed_evolution_entity_fields(
        client,
        config,
        "StageResults",
        &work_item.target_entity_id,
    )
    .await?;
    if !stale_stage_result_should_eliminate(&stage_fields) {
        return Ok(());
    }

    post_directed_evolution_action(
        client,
        config,
        "StageResults",
        &work_item.target_entity_id,
        "EliminateStageResult",
        json!({
            "EliminationRuleId": "stale-after-variant-terminal",
            "EvidenceArtifactId": value_field_string(&stage_fields, &["EvidenceArtifactId", "evidence_artifact_id"]),
            "Reason": reason,
        }),
    )
    .await
}

fn stale_directed_evolution_stage_work_reason(
    work_item: &DirectedEvolutionWorkItemState,
    stage_fields: &Value,
    variant_fields: &Value,
) -> Option<String> {
    if !stale_stage_work_targets_stage_result(work_item) {
        return None;
    }
    let stage_status = value_field_string(stage_fields, &["Status", "status"]);
    if !stage_status.trim().is_empty() && stage_status != "Running" {
        return Some(format!(
            "Target StageResult {} is already {}; skipping stale {} work",
            work_item.target_entity_id, stage_status, work_item.role
        ));
    }
    let variant_status = value_field_string(variant_fields, &["Status", "status"]);
    if matches!(
        variant_status.as_str(),
        "Eliminated" | "Promoted" | "Superseded" | "Failed"
    ) {
        return Some(format!(
            "Target variant is already {}; skipping stale {} work for StageResult {}",
            variant_status, work_item.role, work_item.target_entity_id
        ));
    }
    None
}

fn stale_stage_work_targets_stage_result(work_item: &DirectedEvolutionWorkItemState) -> bool {
    directed_evolution_stage_evaluator_role(&work_item.role)
        && work_item.target_entity_type == "StageResult"
}

fn directed_evolution_stage_evaluator_role(role: &str) -> bool {
    matches!(
        role,
        "reviewer"
            | "viability_evaluator"
            | "state_verifier"
            | "telemetry_evaluator"
            | "wasm_evaluator"
    )
}

fn stale_stage_result_should_eliminate(stage_fields: &Value) -> bool {
    matches!(
        value_field_string(stage_fields, &["Status", "status"]).as_str(),
        "Running" | "Failed"
    )
}
