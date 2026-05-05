async fn connect_and_watch_events(
    client: &reqwest::Client,
    config: &Config,
    url: &str,
) -> Result<()> {
    let response = client
        .get(url)
        .headers(event_stream_headers(config)?)
        .header(ACCEPT, "text/event-stream")
        .send()
        .await
        .with_context(|| format!("connect to event stream {url}"))?;

    if !response.status().is_success() {
        bail!("event stream returned {}", response.status());
    }

    info!(url, "connected to Temper event stream");

    let mut buffer = String::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("read SSE chunk")?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(frame_end) = buffer.find("\n\n") {
            let frame = buffer[..frame_end].to_string();
            buffer = buffer[frame_end + 2..].to_string();
            if let Some(data) = extract_sse_data(&frame) {
                handle_event_payload(client, config, &data).await?;
            }
        }
    }

    Ok(())
}

async fn handle_event_payload(client: &reqwest::Client, config: &Config, data: &str) -> Result<()> {
    let event: EntityEvent = match serde_json::from_str(data) {
        Ok(event) => event,
        Err(error) => {
            debug!(%error, data, "ignoring non-entity SSE payload");
            return Ok(());
        }
    };

    match (event.entity_type.as_str(), event.status.as_str()) {
        ("WorkerRun", "Queued") => {
            handle_queued_worker_run(client, config, &event.entity_id).await?;
        }
        ("ReviewRun", "Requested") => {
            handle_requested_review_run(client, config, &event.entity_id).await?;
        }
        ("EvaluationRun", "Queued") => {
            handle_queued_evaluation_run(client, config, &event.entity_id).await?;
        }
        _ => {}
    }
    Ok(())
}

async fn handle_queued_worker_run(
    client: &reqwest::Client,
    config: &Config,
    worker_run_id: &str,
) -> Result<()> {
    info!(worker_run_id, "saw queued WorkerRun");
    let worker_run = fetch_claimable_worker_run(client, config, worker_run_id).await?;
    if worker_run.status != "Queued" {
        debug!(worker_run_id, status = %worker_run.status, "WorkerRun no longer queued");
        return Ok(());
    }
    if !worker_run_is_claimable_by_local_codex(&worker_run, &config.worker_id) {
        debug!(
            worker_run_id,
            runner_kind = %worker_run.runner_kind,
            allowed_worker_id = %worker_run.allowed_worker_id,
            worker_id = %config.worker_id,
            "WorkerRun is not claimable by local Codex"
        );
        return Ok(());
    }

    claim_worker_run(client, config, worker_run_id).await?;
    start_local_worker_run(client, config, worker_run_id).await?;

    let run_result = if let Some(snapshot_id) = extract_repo_sweep_snapshot_id(&worker_run.task) {
        run_repo_sweep(client, config, &worker_run, &snapshot_id).await
    } else {
        run_codex(config, &worker_run).await
    };

    match run_result {
        Ok(summary) => report_done(client, config, &worker_run, &summary).await,
        Err(error) => {
            error!(worker_run_id, %error, "local Codex run failed");
            report_failed(client, config, worker_run_id, &error.to_string()).await
        }
    }
}

async fn handle_requested_review_run(
    client: &reqwest::Client,
    config: &Config,
    review_run_id: &str,
) -> Result<()> {
    info!(review_run_id, "saw requested ReviewRun");
    let review_run = fetch_configured_review_run(client, config, review_run_id).await?;
    if review_run.status != "Requested" {
        debug!(review_run_id, status = %review_run.status, "ReviewRun no longer requested");
        return Ok(());
    }
    if review_run.worker_run_id.is_empty() {
        debug!(review_run_id, "ReviewRun is not configured yet");
        return Ok(());
    }

    let worker_run = fetch_worker_run(client, config, &review_run.worker_run_id).await?;
    if !worker_run_is_repo_sweep(&worker_run) {
        if !config.enable_execution {
            debug!(
                review_run_id,
                worker_run_id = %worker_run.id,
                "leaving non-repo-sweep ReviewRun for a reviewer agent"
            );
            return Ok(());
        }

        info!(
            review_run_id,
            action = REVIEW_CLAIM_LABEL,
            "claiming code-change ReviewRun for local Codex reviewer"
        );
        post_entity_action(
            client,
            config,
            "ReviewRuns",
            review_run_id,
            "Claim",
            json!({ "reviewer_id": config.worker_id }),
        )
        .await?;
        post_entity_action(
            client,
            config,
            "ReviewRuns",
            review_run_id,
            "StartReview",
            json!({}),
        )
        .await?;

        match run_codex_review(config, &worker_run, &review_run).await {
            Ok(decision) => {
                let body = json!({
                    "review_summary": decision.summary,
                    "live_e2e_summary": decision.live_e2e_summary,
                    "verdict": decision.verdict,
                });
                let action = match decision.action {
                    ReviewDecisionAction::Approve => "Approve",
                    ReviewDecisionAction::RequestChanges => "RequestChanges",
                    ReviewDecisionAction::Escalate => "Escalate",
                };
                return post_entity_action(
                    client,
                    config,
                    "ReviewRuns",
                    review_run_id,
                    action,
                    body,
                )
                .await;
            }
            Err(error) => {
                return post_entity_action(
                    client,
                    config,
                    "ReviewRuns",
                    review_run_id,
                    "Fail",
                    json!({ "review_summary": format!("local Codex review failed: {error}") }),
                )
                .await;
            }
        }
    }

    info!(
        review_run_id,
        action = REVIEW_CLAIM_LABEL,
        "claiming repo-sweep ReviewRun"
    );
    post_entity_action(
        client,
        config,
        "ReviewRuns",
        review_run_id,
        "Claim",
        json!({ "reviewer_id": config.worker_id }),
    )
    .await?;
    post_entity_action(
        client,
        config,
        "ReviewRuns",
        review_run_id,
        "StartReview",
        json!({}),
    )
    .await?;

    let live_e2e_summary = repo_sweep_live_e2e_summary(client, config, &worker_run)
        .await
        .unwrap_or_else(|error| format!("Repo sweep live evidence lookup failed: {error}"));
    let review_summary = format!(
        "Automated repo-sweep reviewer checked WorkerRun {}, proof {}, and repo sweep output. This auto-review only applies to Patrol repo-health sweeps; normal code-change ReviewRuns remain queued for an independent reviewer agent.",
        worker_run.id,
        if review_run.proof_packet_id.is_empty() {
            "(pending proof)"
        } else {
            review_run.proof_packet_id.as_str()
        }
    );

    info!(
        review_run_id,
        action = REVIEW_APPROVE_LABEL,
        "approving repo-sweep ReviewRun"
    );
    post_entity_action(
        client,
        config,
        "ReviewRuns",
        review_run_id,
        "Approve",
        json!({
            "review_summary": review_summary,
            "live_e2e_summary": live_e2e_summary,
            "verdict": "approved_repo_sweep_only"
        }),
    )
    .await
}

async fn handle_queued_evaluation_run(
    client: &reqwest::Client,
    config: &Config,
    evaluation_run_id: &str,
) -> Result<()> {
    info!(evaluation_run_id, "saw queued EvaluationRun");
    let evaluation_run = fetch_configured_evaluation_run(client, config, evaluation_run_id).await?;
    if evaluation_run.status != "Queued" {
        debug!(evaluation_run_id, status = %evaluation_run.status, "EvaluationRun no longer queued");
        return Ok(());
    }
    if evaluation_run.work_cycle_id.is_empty() {
        debug!(evaluation_run_id, "EvaluationRun is not configured yet");
        return Ok(());
    }

    let mut work_cycle = fetch_work_cycle(client, config, &evaluation_run.work_cycle_id).await?;
    if work_cycle.implementer_worker_run_id.is_empty() {
        debug!(evaluation_run_id, work_cycle_id = %work_cycle.id, "WorkCycle has no implementer run yet");
        return Ok(());
    }
    let worker_run =
        fetch_worker_run(client, config, &work_cycle.implementer_worker_run_id).await?;
    let is_repo_sweep = worker_run_is_repo_sweep(&worker_run);
    if !is_repo_sweep && !config.enable_execution {
        debug!(
            evaluation_run_id,
            worker_run_id = %worker_run.id,
            "leaving non-repo-sweep EvaluationRun for evaluator agent"
        );
        return Ok(());
    }

    if !work_cycle.review_passed && !work_cycle.reviewer_run_id.is_empty() {
        handle_requested_review_run(client, config, &work_cycle.reviewer_run_id).await?;
        work_cycle =
            fetch_work_cycle_until_review_passed(client, config, &evaluation_run.work_cycle_id)
                .await?;
    }
    if !work_cycle.review_passed {
        debug!(
            evaluation_run_id,
            work_cycle_id = %work_cycle.id,
            "EvaluationRun is waiting for review_passed"
        );
        return Ok(());
    }

    if !evaluation_run.evaluator_id.is_empty() && evaluation_run.evaluator_id != config.worker_id {
        debug!(
            evaluation_run_id,
            evaluator_id = %evaluation_run.evaluator_id,
            worker_id = %config.worker_id,
            "EvaluationRun is reserved for a different evaluator"
        );
        return Ok(());
    }

    claim_evaluation_run(client, config, evaluation_run_id).await?;

    if !is_repo_sweep {
        return run_code_change_evaluation(client, config, evaluation_run_id, &worker_run).await;
    }

    info!(
        evaluation_run_id,
        action = EVALUATION_START_LABEL,
        "starting repo-sweep EvaluationRun"
    );
    post_entity_action(
        client,
        config,
        "EvaluationRuns",
        evaluation_run_id,
        "Start",
        json!({}),
    )
    .await?;

    let e2e_summary = repo_sweep_live_e2e_summary(client, config, &worker_run)
        .await
        .unwrap_or_else(|error| format!("Repo sweep live evidence lookup failed: {error}"));
    let results_json = serde_json::to_string(&json!({
        "kind": "repo_sweep_evaluation",
        "worker_run_id": worker_run.id,
        "work_cycle_id": work_cycle.id,
        "required_checks": evaluation_run.required_checks,
        "checks": [
            { "name": "worker_done", "status": worker_run.status },
            { "name": "repo_graph_snapshot_ready", "summary": e2e_summary }
        ]
    }))
    .context("serialize repo-sweep evaluation result")?;

    info!(
        evaluation_run_id,
        action = EVALUATION_PASS_LABEL,
        "passing repo-sweep EvaluationRun"
    );
    post_entity_action(
        client,
        config,
        "EvaluationRuns",
        evaluation_run_id,
        "Pass",
        json!({
            "results_json": results_json,
            "e2e_summary": e2e_summary
        }),
    )
    .await
}
