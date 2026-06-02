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
        if directed_evolution_work_item_requires_datadog_evidence(work_item) {
            bail!(
                "Directed Evolution role {} requires live Codex execution and Datadog evidence",
                work_item.role
            );
        }
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
    if directed_evolution_mechanical_evaluator_role(&work_item.role) {
        let payload = run_directed_evolution_mechanical_evaluator(client, config, work_item).await?;
        return serde_json::to_string(&payload)
            .context("serialize Directed Evolution mechanical evaluator output");
    }
    if work_item.role == "promoter" {
        let materialization =
            materialize_directed_evolution_promotion(client, config, work_item).await?;
        return serde_json::to_string(&directed_evolution_promotion_output(&materialization))
            .context("serialize Directed Evolution promoter output");
    }

    let workdir = resolve_directed_evolution_workdir(client, config, work_item).await?;
    let readonly_status_before = if directed_evolution_role_may_write_repo(&work_item.role) {
        None
    } else {
        directed_evolution_git_status_snapshot(&workdir.path).await?
    };
    let output = match if directed_evolution_work_item_requires_datadog_evidence(work_item) {
        run_codex_exec_command_with_datadog_mcp(
            config,
            &workdir.path,
            prompt,
            "run Directed Evolution Datadog-backed role",
        )
        .await
    } else {
        run_codex_exec_command(config, &workdir.path, prompt, "run Directed Evolution Codex role")
            .await
    } {
        Ok(output) => output,
        Err(error) => {
            return recover_directed_evolution_variant_output(
                client, config, work_item, &workdir, error,
            )
            .await;
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        return recover_directed_evolution_variant_output(
            client,
            config,
            work_item,
            &workdir,
            anyhow!(
                "codex role {} failed with status {:?}: {}",
                work_item.role,
                output.status.code(),
                truncate_middle(&format!("{stdout}\n{stderr}"), 4_000)
            ),
        )
        .await;
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
    payload = finalize_directed_evolution_output(client, config, work_item, &workdir, payload).await?;
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
    ensure_directed_evolution_required_datadog_evidence(work_item, &payload)?;
    serde_json::to_string(&payload).context("serialize Directed Evolution Codex output")
}

fn directed_evolution_agent_kind_for_role(role: &str) -> &'static str {
    if matches!(role, "promoter" | "state_verifier" | "wasm_evaluator") {
        "temperpaw-worker"
    } else {
        "codex"
    }
}

fn directed_evolution_role_requires_datadog_mcp(role: &str) -> bool {
    matches!(role, "observer" | "telemetry_evaluator")
}

fn directed_evolution_work_item_requires_datadog_evidence(
    work_item: &DirectedEvolutionWorkItemState,
) -> bool {
    directed_evolution_role_requires_datadog_mcp(&work_item.role)
        || directed_evolution_text_requires_datadog_evidence(&work_item.prompt_ref)
        || directed_evolution_text_requires_datadog_evidence(&work_item.context_ref)
        || directed_evolution_text_requires_datadog_evidence(&work_item.output_schema_ref)
        || directed_evolution_text_requires_datadog_evidence(&work_item.correlation_json)
}

fn directed_evolution_text_requires_datadog_evidence(value: &str) -> bool {
    value
        .to_ascii_lowercase()
        .contains("datadog_evidence_scope")
}

fn directed_evolution_worker_provider_id() -> String {
    worker_provider_id()
}

fn directed_evolution_model_for_role(role: &str) -> String {
    if matches!(role, "promoter" | "state_verifier" | "wasm_evaluator") {
        "deterministic-worker".to_string()
    } else {
        directed_evolution_codex_model_label()
    }
}

fn directed_evolution_codex_model_label() -> String {
    env::var("PAW_CODEX_MODEL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "codex-cli".to_string())
}

async fn recover_directed_evolution_variant_output(
    client: &reqwest::Client,
    config: &Config,
    work_item: &DirectedEvolutionWorkItemState,
    workdir: &DirectedEvolutionWorkdir,
    error: anyhow::Error,
) -> Result<String> {
    if work_item.role != "variant_generator"
        || env_flag("PAW_DE_RECOVER_VARIANT_CHANGES_ON_CODEX_ERROR") == Some(false)
    {
        return Err(error);
    }
    let changed_files = match git_changed_files(&workdir.path).await {
        Ok(files) if !files.is_empty() => files,
        _ => return Err(error),
    };
    let error_summary = truncate_middle(&error.to_string(), 1_200);
    warn!(
        work_item_id = %work_item.id,
        changed_file_count = changed_files.len(),
        error = %error_summary,
        "recovering Directed Evolution variant from git changes after Codex execution error"
    );
    let payload = json!({
        "status": "recovered_after_codex_exec_error",
        "summary": format!(
            "Recovered a Directed Evolution variant from {} changed file(s) after Codex exited before returning JSON.",
            changed_files.len()
        ),
        "changed_files": changed_files,
        "verification_notes": format!(
            "Codex produced repository changes but did not finish the structured response: {error_summary}. Later evaluation stages must validate the hot-loaded runtime before selection."
        ),
        "reasoning_summary": "The worker recovered concrete file mutations left by the variant-generator worker run and will publish them as a candidate for downstream evaluation.",
        "recovery": {
            "reason": "codex_exec_error_after_file_changes",
            "error": error_summary,
            "worker_id": config.worker_id,
        }
    });
    let payload =
        finalize_directed_evolution_output(client, config, work_item, workdir, payload).await?;
    serde_json::to_string(&payload).context("serialize recovered Directed Evolution variant output")
}
