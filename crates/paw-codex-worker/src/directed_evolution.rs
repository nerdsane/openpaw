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
        debug!(work_item_id, "Directed Evolution WorkItem has no brain role yet");
        return Ok(());
    }

    let brain_run_id = create_entity(client, config, "BrainRuns", json!({})).await?;
    post_directed_evolution_action(
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
    .await?;
    post_directed_evolution_action(
        client,
        config,
        "BrainRuns",
        &brain_run_id,
        "StartBrainRun",
        json!({
            "Role": work_item.role,
            "WorkItemId": work_item.id,
            "AgentKind": "codex",
            "Model": "codex-cli",
            "ParentSessionId": env::var("CODEX_SESSION_ID").unwrap_or_default(),
            "CorrelationJson": work_item.correlation_json,
        }),
    )
    .await?;
    post_directed_evolution_action(
        client,
        config,
        "WorkItems",
        &work_item.id,
        "StartWorkItem",
        json!({ "BrainRunId": brain_run_id }),
    )
    .await?;
    info!(
        work_item_id = %work_item.id,
        brain_run_id = %brain_run_id,
        role = %work_item.role,
        target_entity_type = %work_item.target_entity_type,
        target_entity_id = %work_item.target_entity_id,
        "started Directed Evolution Codex brain run"
    );

    match run_directed_evolution_codex_role(client, config, &work_item).await {
        Ok(output_json) => {
            let summary = directed_evolution_summary(&work_item, &output_json);
            let evidence_artifact_id = record_directed_evolution_brain_evidence(
                client,
                config,
                &work_item,
                &brain_run_id,
                "codex_brain_run",
                &output_json,
                &summary,
            )
            .await?;
            post_directed_evolution_action(
                client,
                config,
                "BrainRuns",
                &brain_run_id,
                "SucceedBrainRun",
                json!({
                    "OutputJson": output_json,
                    "EvidenceArtifactId": evidence_artifact_id,
                    "Summary": summary,
                }),
            )
            .await?;
            post_directed_evolution_action(
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
                brain_run_id = %brain_run_id,
                role = %work_item.role,
                evidence_artifact_id = %evidence_artifact_id,
                "completed Directed Evolution Codex brain run"
            );
            Ok(())
        }
        Err(error) => {
            let failure_reason = format!("Directed Evolution Codex role failed: {error}");
            let evidence_artifact_id = match record_directed_evolution_brain_evidence(
                client,
                config,
                &work_item,
                &brain_run_id,
                "codex_brain_run_failure",
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
                    warn!(%report_error, work_item_id, brain_run_id, "failed to record Directed Evolution failure evidence");
                    String::new()
                }
            };
            if let Err(report_error) = post_directed_evolution_action(
                client,
                config,
                "BrainRuns",
                &brain_run_id,
                "FailBrainRun",
                json!({
                    "FailureReason": failure_reason,
                    "EvidenceArtifactId": evidence_artifact_id,
                }),
            )
            .await
            {
                warn!(%report_error, work_item_id, brain_run_id, "failed to report BrainRun failure");
            }
            post_directed_evolution_action(
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
                brain_run_id = %brain_run_id,
                role = %work_item.role,
                evidence_artifact_id = %evidence_artifact_id,
                "failed Directed Evolution Codex brain run"
            );
            Ok(())
        }
    }
}


include!("directed_evolution/evidence.rs");

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

async fn run_directed_evolution_codex_role(
    client: &reqwest::Client,
    config: &Config,
    work_item: &DirectedEvolutionWorkItemState,
) -> Result<String> {
    let prompt = directed_evolution_prompt(work_item);
    info!(
        work_item_id = %work_item.id,
        role = %work_item.role,
        target_entity_type = %work_item.target_entity_type,
        target_entity_id = %work_item.target_entity_id,
        execution_enabled = config.enable_execution,
        "executing Directed Evolution Codex role"
    );
    if !config.enable_execution {
        return serde_json::to_string(&json!({
            "status": "dry_run",
            "role": work_item.role,
            "work_item_id": work_item.id,
            "target": {
                "entity_type": work_item.target_entity_type,
                "entity_id": work_item.target_entity_id,
            },
            "prompt_preview": truncate_middle(&prompt, 1200),
        }))
            .context("serialize Directed Evolution dry-run output");
    }

    let workdir = resolve_directed_evolution_workdir(client, config, work_item).await?;
    let readonly_status_before = if directed_evolution_role_may_write_repo(&work_item.role) {
        None
    } else {
        directed_evolution_git_status_snapshot(&workdir.path).await?
    };
    let output = run_codex_exec_command(
        config,
        &workdir.path,
        prompt,
        "run Directed Evolution Codex role",
    )
    .await?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        bail!(
            "codex role {} failed with status {:?}: {}",
            work_item.role,
            output.status.code(),
            truncate_middle(&format!("{stdout}\n{stderr}"), 4_000)
        );
    }
    let mut payload = parse_codex_jsonish(&stdout).unwrap_or_else(|| {
        json!({
            "status": "succeeded",
            "summary": truncate_middle(&stdout, 4_000),
        })
    });
    if let Some(status_before) = readonly_status_before {
        ensure_directed_evolution_readonly_workdir_unchanged(&workdir.path, &status_before).await?;
    }
    payload = finalize_directed_evolution_output(work_item, &workdir, payload).await?;
    if let Some(object) = payload.as_object_mut() {
        object
            .entry("role".to_string())
            .or_insert_with(|| json!(work_item.role));
        object
            .entry("work_item_id".to_string())
            .or_insert_with(|| json!(work_item.id));
        object.entry("target".to_string()).or_insert_with(|| {
            json!({
                "entity_type": work_item.target_entity_type,
                "entity_id": work_item.target_entity_id,
            })
        });
        object.entry("execution".to_string()).or_insert_with(|| {
            json!({
                "workdir": workdir.path.display().to_string(),
                "stdout_bytes": stdout.len(),
                "stderr_bytes": stderr.len(),
            })
        });
    }
    serde_json::to_string(&payload)
    .context("serialize Directed Evolution Codex output")
}


include!("directed_evolution/workdir.rs");
include!("directed_evolution/prompt.rs");
include!("directed_evolution/tests.rs");
