fn directed_evolution_prompt(work_item: &DirectedEvolutionWorkItemState) -> String {
    let role_contract = match work_item.role.as_str() {
        "observer" => {
            "Observe real signals and infer pressures. If the signal source or correlation mentions Datadog, use authenticated Datadog MCP tools to inspect the relevant logs, traces, metrics, monitors, or dashboards. Do not treat the signal summary as proof. Return candidate pressures, rejected interpretations, structured evidence_scope entries with datadog_url values when available, and confidence."
        }
        "direction_framer" => {
            "Frame human-legible directions from pressures. Return direction title, rationale, source signals, required human gate, and likely adaptation goals."
        }
        "variant_generator" => {
            "Generate one bounded candidate variant for the target organism. Make the concrete app-bundle changes, avoid long exploratory verification, and return the concise JSON object immediately after the mutation is complete."
        }
        "simulated_user" => {
            "Act as an AI simulated user against the target organism. Exercise the live runtime when a RuntimeRef is provided. If RequiredEvidence includes datadog_evidence_scope, use authenticated Datadog MCP tools to inspect errors or latency evidence tied to that tenant/app, include a Datadog URL in evidence_scope, and return passed=false with a clear failure_reason if Datadog cannot be queried. Return goals attempted, observations, unmet intents, metrics, traces, and structured evidence_scope entries."
        }
        "reviewer" => {
            "Review a variant against the adaptation goal and viability constraints. If RequiredEvidence includes datadog_evidence_scope, use authenticated Datadog MCP tools for logs, traces, or metrics evidence tied to the variant tenant/app, include a Datadog URL in evidence_scope, and return passed=false with a clear failure_reason if Datadog cannot be queried. Return pass/fail reasoning, metrics, structured evidence_scope entries, and risk notes."
        }
        "selector" => {
            "Select a winner from supplied evaluated-variant evidence without changing files, evaluators, or moving goalposts. Return winner, losers, scores, and selection rationale."
        }
        "promoter" => {
            "Materialize an already-selected promotion into the canonical Genesis runtime. Do not choose a winner; publish and hot-load the selected app ref."
        }
        "narrator" => {
            "Explain the episode outcome for Mission Control. Return concise human-facing narrative, lineage impact, and evidence links."
        }
        _ => {
            "Execute the Directed Evolution brain role described by this WorkItem. Return structured evidence and next-state recommendations."
        }
    };
    let output_contract = directed_evolution_output_contract(&work_item.role);
    let prompt_body =
        directed_evolution_worker_prompt_body(&work_item.role, &literal_prompt_ref(&work_item.prompt_ref));
    format!(
        r#"You are a Codex brain run executing a Directed Evolution WorkItem.

Role: {role}
WorkItemId: {work_item_id}
TargetEntityType: {target_entity_type}
TargetEntityId: {target_entity_id}
ContextRef: {context_ref}
OutputSchemaRef: {output_schema_ref}
CorrelationJson: {correlation_json}

Role contract:
{role_contract}

Prompt:
{prompt_body}

Return exactly one concise JSON object. Do not wrap it in Markdown.
Required output shape for this role:
{output_contract}
"#,
        role = work_item.role,
        work_item_id = work_item.id,
        target_entity_type = work_item.target_entity_type,
        target_entity_id = work_item.target_entity_id,
        context_ref = work_item.context_ref,
        output_schema_ref = work_item.output_schema_ref,
        correlation_json = work_item.correlation_json,
        role_contract = role_contract,
        prompt_body = prompt_body,
        output_contract = output_contract,
    )
}

fn directed_evolution_output_contract(role: &str) -> &'static str {
    match role {
        "observer" => {
            r#"{
  "actionable": true,
  "pressure_class": "growth|repair|performance|policy|ux",
  "pressure_summary": "...",
  "title": "...",
  "direction_summary": "...",
  "autonomy_lane": "human-approval|repair-auto",
  "proposed_adaptation_goal": "...",
  "proposed_viability_constraints": ["..."],
  "evidence_scope": [{"surface":"logs|traces|metrics|monitors|runtime","query":"...","result_summary":"...","datadog_url":"https://app.datadoghq.com/..."}],
  "evidence_refs": ["..."],
  "reasoning_summary": "..."
}"#
        }
        "variant_generator" => {
            r#"{
  "summary": "...",
  "app_ref": "repository-or-runtime-ref",
  "branch_ref": "branch-or-worktree-ref",
  "runtime_ref": "runnable variant URL or local ref",
  "changed_files": ["..."],
  "diff_ref": "...",
  "verification_notes": "...",
  "reasoning_summary": "..."
}"#
        }
        "simulated_user" | "reviewer" => {
            r#"{
  "passed": true,
  "status": "passed|failed",
  "summary": "...",
  "metrics": {"metric_name": 0},
  "evidence_scope": [{"surface":"runtime|logs|traces|metrics","query":"...","result_summary":"...","datadog_url":"https://app.datadoghq.com/..."}],
  "evidence_refs": ["..."],
  "failure_reason": "",
  "reasoning_summary": "..."
}"#
        }
        "selector" => {
            r#"{
  "winning_variant_id": "...",
  "selection_explanation": "...",
  "app_ref": "...",
  "commit_ref": "...",
  "evidence_uri": "...",
  "digest": "...",
  "tradeoffs": ["..."],
  "reasoning_summary": "..."
}"#
        }
        "promoter" => {
            r#"{
  "status": "succeeded|failed",
  "canonical_app_ref": "owner/app@hash",
  "production_tenant": "default",
  "runtime_ref": "temper://tenant/default/app/owner/app@hash",
  "summary": "...",
  "evidence_refs": ["..."],
  "digest": "...",
  "reasoning_summary": "..."
}"#
        }
        _ => {
            r#"{
  "status": "succeeded|failed",
  "role": "...",
  "work_item_id": "...",
  "evidence_refs": ["..."],
  "reasoning_summary": "...",
  "next_actions": ["..."]
}"#
        }
    }
}

fn literal_prompt_ref(prompt_ref: &str) -> String {
    prompt_ref
        .strip_prefix("literal:")
        .unwrap_or(prompt_ref)
        .trim()
        .to_string()
}

fn directed_evolution_worker_prompt_body(role: &str, prompt_body: &str) -> String {
    if !matches!(role, "reviewer" | "simulated_user") {
        return prompt_body.to_string();
    }

    let mut body = prompt_body.to_string();
    if let Some(public_api_url) = directed_evolution_public_api_url() {
        body = rewrite_temper_api_base(&body, &public_api_url);
    }
    body.push_str(
        "\n\nRuntime execution discipline:\n\
- Prefer the TemperApiBase URL above with the tenant parsed from RuntimeRef; do not assume localhost is the target runtime.\n\
- Do not start a foreground long-lived server. If a local server is absolutely required, start it in the background, stop it before returning, and include the cleanup in evidence_scope.\n\
- If runtime execution is unavailable, fail the stage with clear evidence instead of hanging.\n\
- If RequiredEvidence includes datadog_evidence_scope, Datadog evidence is mandatory; do not pass with only local/runtime evidence.\n",
    );
    body
}

fn directed_evolution_public_api_url() -> Option<String> {
    env::var("DIRECTED_EVOLUTION_PUBLIC_API_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(directed_evolution_genesis_url)
}

fn rewrite_temper_api_base(prompt_body: &str, public_api_url: &str) -> String {
    prompt_body
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("TemperApiBase:")
                && (line.contains("127.0.0.1") || line.contains("localhost"))
            {
                format!("TemperApiBase: {}", public_api_url.trim_end_matches('/'))
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn directed_evolution_summary(
    work_item: &DirectedEvolutionWorkItemState,
    output_json: &str,
) -> String {
    let output_summary = serde_json::from_str::<Value>(output_json)
        .ok()
        .and_then(|value| {
            value
                .get("summary")
                .or_else(|| value.get("reasoning_summary"))
                .or_else(|| value.get("selection_explanation"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        });
    if let Some(summary) = output_summary {
        return format!(
            "Directed Evolution {} WorkItem {} completed: {}",
            work_item.role,
            work_item.id,
            truncate_middle(&summary, 600)
        );
    }
    format!(
        "Directed Evolution {} WorkItem {} completed for {}:{} ({} bytes output).",
        work_item.role,
        work_item.id,
        work_item.target_entity_type,
        work_item.target_entity_id,
        output_json.len()
    )
}
