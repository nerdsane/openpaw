async fn handle_queued_directed_evolution_work_item(
    client: &reqwest::Client,
    config: &Config,
    work_item_id: &str,
) -> Result<()> {
    info!(work_item_id, "saw queued Directed Evolution WorkItem");
    let work_item = fetch_directed_evolution_work_item(client, config, work_item_id).await?;
    if work_item.status != "Queued" {
        debug!(
            work_item_id,
            status = %work_item.status,
            "Directed Evolution WorkItem no longer queued"
        );
        return Ok(());
    }
    if work_item.role.trim().is_empty() {
        debug!(work_item_id, "Directed Evolution WorkItem has no worker role yet");
        return Ok(());
    }
    if let Some(conflict) =
        directed_evolution_exclusive_key_conflict(client, config, &work_item).await?
    {
        let reason = format!(
            "Exclusive key {} is already active on WorkItem {}; cancelling duplicate {} work",
            conflict.exclusive_key, conflict.active_work_item_id, work_item.role
        );
        post_paw_orchestration_action(
            client,
            config,
            "WorkItems",
            &work_item.id,
            "CancelWorkItem",
            json!({ "Reason": reason }),
        )
        .await?;
        info!(
            work_item_id = %work_item.id,
            active_work_item_id = %conflict.active_work_item_id,
            exclusive_key = %conflict.exclusive_key,
            "cancelled duplicate exclusive Directed Evolution WorkItem"
        );
        return Ok(());
    }
    if let Some(reason) =
        stale_directed_evolution_work_item_reason(client, config, &work_item).await?
    {
        if let Err(error) =
            eliminate_stale_directed_evolution_stage_result(client, config, &work_item, &reason)
                .await
        {
            warn!(
                work_item_id = %work_item.id,
                role = %work_item.role,
                %error,
                "stale Directed Evolution WorkItem cleanup could not mutate StageResult; cancelling WorkItem only"
            );
        }
        if let Err(error) = post_paw_orchestration_action(
            client,
            config,
            "WorkItems",
            &work_item.id,
            "CancelWorkItem",
            json!({ "Reason": reason }),
        )
        .await
            && !error.to_string().contains("not valid from state 'Cancelled'")
        {
            return Err(error);
        }
        info!(
            work_item_id = %work_item.id,
            role = %work_item.role,
            "cancelled stale Directed Evolution WorkItem"
        );
        return Ok(());
    }

    if let Err(error) = post_paw_orchestration_action(
        client,
        config,
        "WorkItems",
        &work_item.id,
        "ClaimWorkItem",
        json!({
            "WorkerId": config.worker_id,
            "ClaimedBy": config.worker_id,
        }),
    )
    .await
    {
        let message = error.to_string();
        if directed_evolution_claim_conflict_message(&message) {
            let current = fetch_directed_evolution_work_item(client, config, &work_item.id).await?;
            info!(
                work_item_id = %work_item.id,
                role = %work_item.role,
                status = %current.status,
                "Directed Evolution WorkItem claim lost to another worker"
            );
            if current.status != "Queued" {
                return Ok(());
            }
        }
        return Err(error).with_context(|| {
            format!(
                "claim Directed Evolution WorkItem {} as {}",
                work_item.id, config.worker_id
            )
        });
    }
    if let Some(conflict) =
        directed_evolution_exclusive_key_post_claim_conflict(client, config, &work_item).await?
    {
        let reason = format!(
            "Exclusive key {} is already claimed by lower-priority WorkItem {}; cancelling duplicate {} work before WorkerRun start",
            conflict.exclusive_key, conflict.active_work_item_id, work_item.role
        );
        post_paw_orchestration_action(
            client,
            config,
            "WorkItems",
            &work_item.id,
            "CancelWorkItem",
            json!({ "Reason": reason }),
        )
        .await?;
        info!(
            work_item_id = %work_item.id,
            active_work_item_id = %conflict.active_work_item_id,
            exclusive_key = %conflict.exclusive_key,
            "cancelled duplicate exclusive Directed Evolution WorkItem after claim"
        );
        return Ok(());
    }

    let worker_run_id = create_entity(client, config, "WorkerRuns", json!({})).await?;
    post_paw_orchestration_action(
        client,
        config,
        "WorkerRuns",
        &worker_run_id,
        "StartWorkerRun",
        json!({
            "Role": work_item.role,
            "WorkItemId": work_item.id,
            "WorkerId": config.worker_id,
            "ProviderId": directed_evolution_worker_provider_id(),
            "AgentKind": directed_evolution_agent_kind_for_role(&work_item.role),
            "Model": directed_evolution_model_for_role(&work_item.role),
            "SessionId": "",
            "ParentSessionId": env::var("CODEX_SESSION_ID").unwrap_or_default(),
            "CorrelationJson": work_item.correlation_json,
        }),
    )
    .await?;
    post_paw_orchestration_action(
        client,
        config,
        "WorkItems",
        &work_item.id,
        "StartWorkItem",
        json!({ "WorkerRunId": worker_run_id }),
    )
    .await?;
    info!(
        work_item_id = %work_item.id,
        worker_run_id = %worker_run_id,
        role = %work_item.role,
        target_entity_type = %work_item.target_entity_type,
        target_entity_id = %work_item.target_entity_id,
        "started Directed Evolution worker run"
    );

    match run_directed_evolution_codex_role(client, config, &work_item).await {
        Ok(output_json) => {
            let summary = directed_evolution_summary(&work_item, &output_json);
            let evidence_artifact_id = record_directed_evolution_worker_evidence(
                client,
                config,
                &work_item,
                &worker_run_id,
                "codex_worker_run",
                &output_json,
                &summary,
            )
            .await?;
            post_paw_orchestration_action(
                client,
                config,
                "WorkerRuns",
                &worker_run_id,
                "SucceedWorkerRun",
                json!({
                    "OutputJson": output_json,
                    "EvidenceArtifactId": evidence_artifact_id,
                    "Summary": summary,
                }),
            )
            .await?;
            if let Err(route_error) = route_directed_evolution_work_item_success(
                client,
                config,
                &work_item,
                &worker_run_id,
                &output_json,
                &evidence_artifact_id,
                &summary,
            )
            .await
            {
                let failure_reason = format!(
                    "Directed Evolution receipt routing failed after WorkerRun success: {route_error}"
                );
                warn!(
                    work_item_id = %work_item.id,
                    worker_run_id = %worker_run_id,
                    role = %work_item.role,
                    evidence_artifact_id = %evidence_artifact_id,
                    %route_error,
                    "Directed Evolution WorkItem receipt routing failed after WorkerRun success"
                );
                post_paw_orchestration_action(
                    client,
                    config,
                    "WorkItems",
                    &work_item.id,
                    "FailWorkItem",
                    json!({
                        "FailureReason": failure_reason,
                        "EvidenceArtifactId": evidence_artifact_id,
                    }),
                )
                .await?;
                return Ok(());
            }
            post_paw_orchestration_action(
                client,
                config,
                "WorkItems",
                &work_item.id,
                "SucceedWorkItem",
                json!({
                    "ResultJson": output_json,
                    "EvidenceArtifactId": evidence_artifact_id,
                    "Summary": summary,
                }),
            )
            .await?;
            info!(
                work_item_id = %work_item.id,
                worker_run_id = %worker_run_id,
                role = %work_item.role,
                evidence_artifact_id = %evidence_artifact_id,
                "completed Directed Evolution Codex worker run"
            );
            Ok(())
        }
        Err(error) => {
            let failure_reason = format!("Directed Evolution Codex role failed: {error}");
            let evidence_artifact_id = match record_directed_evolution_worker_evidence(
                client,
                config,
                &work_item,
                &worker_run_id,
                "codex_worker_run_failure",
                &serde_json::to_string(&json!({
                    "status": "failed",
                    "failure_reason": failure_reason,
                }))?,
                &failure_reason,
            )
            .await
            {
                Ok(id) => id,
                Err(report_error) => {
                    warn!(%report_error, work_item_id, worker_run_id, "failed to record Directed Evolution failure evidence");
                    String::new()
                }
            };
            if let Err(report_error) = post_paw_orchestration_action(
                client,
                config,
                "WorkerRuns",
                &worker_run_id,
                "FailWorkerRun",
                json!({
                    "FailureReason": failure_reason,
                    "EvidenceArtifactId": evidence_artifact_id,
                }),
            )
            .await
            {
                warn!(%report_error, work_item_id, worker_run_id, "failed to report WorkerRun failure");
            }
            if let Err(route_error) = route_directed_evolution_work_item_failure(
                client,
                config,
                &work_item,
                &worker_run_id,
                &failure_reason,
                &evidence_artifact_id,
            )
            .await
            {
                warn!(
                    work_item_id = %work_item.id,
                    worker_run_id = %worker_run_id,
                    role = %work_item.role,
                    evidence_artifact_id = %evidence_artifact_id,
                    %route_error,
                    "Directed Evolution failure receipt routing failed; failing WorkItem anyway"
                );
            }
            post_paw_orchestration_action(
                client,
                config,
                "WorkItems",
                &work_item.id,
                "FailWorkItem",
                json!({
                    "FailureReason": failure_reason,
                    "EvidenceArtifactId": evidence_artifact_id,
                }),
            )
            .await?;
            warn!(
                work_item_id = %work_item.id,
                worker_run_id = %worker_run_id,
                role = %work_item.role,
                evidence_artifact_id = %evidence_artifact_id,
                "failed Directed Evolution Codex worker run"
            );
            Ok(())
        }
    }
}
