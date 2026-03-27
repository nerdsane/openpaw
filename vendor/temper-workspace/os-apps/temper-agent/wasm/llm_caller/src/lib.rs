//! LLM Caller — WASM module for calling LLM providers (Anthropic/OpenRouter/Mock).
//!
//! Reads conversation from TemperFS File entity (via $value endpoint) when
//! `conversation_file_id` is set, otherwise falls back to inline entity state.
//! Calls the LLM, appends the response, writes back to TemperFS, and returns
//! a dynamic callback action based on the LLM's response:
//! - `ProcessToolCalls` if the response contains tool_use blocks
//! - `RecordResult` if the response is an end_turn
//! - `Fail` if the turn budget is exceeded
//!
//! Supported modes:
//! - Anthropic API key (`x-api-key`)
//! - Anthropic OAuth token (`Authorization: Bearer sk-ant-oat...`)
//! - OpenRouter API key (`Authorization: Bearer`, OpenAI-compatible schema)
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// Entry point — NOT using `temper_module!` because we need dynamic callback actions.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "llm_caller: starting");

        // Read entity state
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        // Check turn budget
        let turn_count = fields
            .get("turn_count")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let max_turns = fields
            .get("max_turns")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(20);

        if turn_count >= max_turns {
            set_success_result(
                "Fail",
                &json!({ "error_message": format!("turn budget exhausted ({turn_count}/{max_turns})") }),
            );
            return Ok(());
        }

        // Read configuration
        let model = fields
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("claude-sonnet-4-20250514");
        let provider_raw = fields
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or("anthropic");
        let provider = normalize_provider(provider_raw);
        let tools_enabled = fields
            .get("tools_enabled")
            .and_then(|v| v.as_str())
            .unwrap_or("read,write,edit,bash");
        // `system_prompt` is the Anthropic API system parameter (agent persona/behavior).
        // `user_message` is the actual user task from the Provision action.
        let system_prompt = fields
            .get("system_prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let user_message = fields
            .get("user_message")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let sandbox_url = fields
            .get("sandbox_url")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let workdir = fields
            .get("workdir")
            .and_then(|v| v.as_str())
            .unwrap_or("/workspace");

        // Resolve provider credentials from integration config.
        let api_key = if provider == "mock" {
            String::new()
        } else {
            resolve_provider_api_key(&ctx, &provider)?
        };
        if provider != "mock" && is_unresolved_secret_template(&api_key) {
            return Err(format!(
                "provider={provider} api key is unresolved secret template: '{api_key}'. \
set tenant secret and retry"
            ));
        }
        let anthropic_api_url = ctx
            .config
            .get("anthropic_api_url")
            .cloned()
            .unwrap_or_else(|| "https://api.anthropic.com/v1/messages".to_string());
        let openrouter_api_url = ctx
            .config
            .get("openrouter_api_url")
            .cloned()
            .unwrap_or_else(|| "https://openrouter.ai/api/v1/chat/completions".to_string());
        let anthropic_auth_mode = ctx
            .config
            .get("anthropic_auth_mode")
            .cloned()
            .unwrap_or_else(|| "auto".to_string());
        let openrouter_site_url = ctx
            .config
            .get("openrouter_site_url")
            .cloned()
            .unwrap_or_default();
        let openrouter_app_name = ctx
            .config
            .get("openrouter_app_name")
            .cloned()
            .unwrap_or_else(|| "temper-agent".to_string());

        if provider != "mock" && api_key.is_empty() {
            return Err(format!(
                "missing API key for provider={provider}. expected secrets: \
anthropic_api_key (or api_key) for anthropic, openrouter_api_key (or api_key) for openrouter"
            ));
        }

        // TemperFS conversation storage
        let conversation_file_id = fields
            .get("conversation_file_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let temper_api_url = resolve_temper_api_url(&ctx, &fields);
        let tenant = &ctx.tenant;

        // Read conversation — from TemperFS if file_id set, else inline state.
        // First turn uses `user_message` (the actual user task from Provision).
        // `system_prompt` is always sent as the Anthropic system parameter, never as a message.
        if user_message.is_empty() {
            return Err("user_message is empty — nothing to send to the LLM".to_string());
        }
        let first_turn_content = user_message;
        let mut messages: Vec<Value> = if !conversation_file_id.is_empty() {
            read_conversation_from_temperfs(
                &ctx,
                &temper_api_url,
                tenant,
                conversation_file_id,
                first_turn_content,
            )?
        } else {
            let conversation_json = fields
                .get("conversation")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if conversation_json.is_empty() {
                vec![json!({ "role": "user", "content": first_turn_content })]
            } else {
                serde_json::from_str(conversation_json).unwrap_or_else(|_| {
                    vec![json!({ "role": "user", "content": first_turn_content })]
                })
            }
        };

        // Build tool definitions based on tools_enabled
        let tools = build_tool_definitions(tools_enabled, sandbox_url, workdir);

        // Call LLM API
        let response = match provider.as_str() {
            "mock" => call_mock(&ctx, &messages)?,
            "anthropic" => call_anthropic(
                &ctx,
                &api_key,
                &anthropic_api_url,
                model,
                system_prompt,
                &messages,
                &tools,
                &anthropic_auth_mode,
            )?,
            "openrouter" => call_openrouter(
                &ctx,
                &api_key,
                &openrouter_api_url,
                model,
                system_prompt,
                &messages,
                &tools,
                &openrouter_site_url,
                &openrouter_app_name,
            )?,
            other => return Err(format!("unsupported LLM provider: {other}")),
        };

        ctx.log(
            "info",
            &format!(
                "llm_caller: got response, stop_reason={}",
                response.stop_reason
            ),
        );

        // Append assistant response to conversation
        messages.push(json!({
            "role": "assistant",
            "content": response.content,
        }));

        // Write updated conversation to TemperFS (if file_id set) or pass inline
        let updated_conversation = serde_json::to_string(&messages).unwrap_or_default();

        if !conversation_file_id.is_empty() {
            write_conversation_to_temperfs(
                &ctx,
                &temper_api_url,
                tenant,
                conversation_file_id,
                &updated_conversation,
            )?;
        }

        // For TemperFS mode, don't pass conversation inline (it's in the File)
        let conv_param = if conversation_file_id.is_empty() {
            Some(updated_conversation.clone())
        } else {
            None
        };

        // Route based on stop_reason
        match response.stop_reason.as_str() {
            "tool_use" => {
                // Extract tool_use blocks
                let tool_calls: Vec<Value> = response
                    .content
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .filter(|block| block.get("type").and_then(|v| v.as_str()) == Some("tool_use"))
                    .cloned()
                    .collect();

                let tool_calls_json = serde_json::to_string(&tool_calls).unwrap_or_default();
                let mut params = json!({
                    "pending_tool_calls": tool_calls_json,
                    "input_tokens": response.input_tokens,
                    "output_tokens": response.output_tokens,
                });
                if let Some(ref conv) = conv_param {
                    params["conversation"] = json!(conv);
                }
                set_success_result("ProcessToolCalls", &params);
            }
            "end_turn" | "stop" => {
                // Extract text result
                let result_text = response
                    .content
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .filter_map(|block| {
                        if block.get("type").and_then(|v| v.as_str()) == Some("text") {
                            block.get("text").and_then(|v| v.as_str()).map(String::from)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                let mut params = json!({
                    "result": result_text,
                    "input_tokens": response.input_tokens,
                    "output_tokens": response.output_tokens,
                });
                if let Some(ref conv) = conv_param {
                    params["conversation"] = json!(conv);
                }
                set_success_result("RecordResult", &params);
            }
            other => {
                set_success_result(
                    "Fail",
                    &json!({ "error_message": format!("unexpected stop_reason: {other}") }),
                );
            }
        }

        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

/// Parsed LLM response.
struct LlmResponse {
    content: Value,
    stop_reason: String,
    input_tokens: i64,
    output_tokens: i64,
}

fn normalize_provider(provider: &str) -> String {
    let norm = provider.trim().to_ascii_lowercase();
    if norm == "open_router" {
        "openrouter".to_string()
    } else {
        norm
    }
}

fn is_unresolved_secret_template(value: &str) -> bool {
    value.contains("{secret:")
}

fn first_non_empty(values: &[Option<String>]) -> String {
    for v in values.iter().flatten() {
        if !v.trim().is_empty() {
            return v.trim().to_string();
        }
    }
    String::new()
}

fn resolve_provider_api_key(ctx: &Context, provider: &str) -> Result<String, String> {
    let key = match provider {
        "anthropic" => first_non_empty(&[
            ctx.config.get("anthropic_api_key").cloned(),
            ctx.config.get("api_key").cloned(),
        ]),
        "openrouter" => first_non_empty(&[
            ctx.config.get("openrouter_api_key").cloned(),
            ctx.config.get("api_key").cloned(),
        ]),
        other => return Err(format!("unsupported LLM provider: {other}")),
    };
    Ok(key)
}

fn call_mock(ctx: &Context, messages: &[Value]) -> Result<LlmResponse, String> {
    ctx.log("info", "llm_caller: using deterministic mock provider");
    let signal_summary = extract_mock_signal_summary(messages)?;
    let analysis = build_mock_analysis(&signal_summary);
    let analysis_text = serde_json::to_string_pretty(&analysis)
        .map_err(|e| format!("failed to serialize mock analysis: {e}"))?;

    Ok(LlmResponse {
        content: json!([{
            "type": "text",
            "text": analysis_text,
        }]),
        stop_reason: "end_turn".to_string(),
        input_tokens: messages
            .iter()
            .map(|message| {
                message
                    .get("content")
                    .map(stringify_content)
                    .unwrap_or_default()
                    .len() as i64
            })
            .sum::<i64>(),
        output_tokens: analysis_text.len() as i64,
    })
}

fn extract_mock_signal_summary(messages: &[Value]) -> Result<Value, String> {
    for message in messages.iter().rev() {
        if message.get("role").and_then(Value::as_str) != Some("user") {
            continue;
        }
        let raw = message
            .get("content")
            .map(stringify_content)
            .unwrap_or_default();
        if raw.trim().is_empty() {
            continue;
        }
        return serde_json::from_str::<Value>(&raw)
            .map_err(|e| format!("mock provider expected JSON signal summary: {e}"));
    }
    Err("mock provider could not find a user JSON payload".to_string())
}

fn build_mock_analysis(signal_summary: &Value) -> Value {
    let legacy_unmet_intents = signal_summary
        .get("legacy_unmet_intents")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let intent_candidates = signal_summary
        .get("intent_evidence")
        .and_then(|value| value.get("intent_candidates"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let workaround_patterns = signal_summary
        .get("intent_evidence")
        .and_then(|value| value.get("workaround_patterns"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let abandonment_patterns = signal_summary
        .get("intent_evidence")
        .and_then(|value| value.get("abandonment_patterns"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let policy_suggestions = signal_summary
        .get("policy_suggestions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let feature_requests = signal_summary
        .get("feature_requests")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let agents = signal_summary
        .get("agents")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut existing_keys = collect_existing_dedupe_keys(signal_summary);
    let mut findings = Vec::<Value>::new();

    for candidate in intent_candidates.iter().take(4) {
        let issue_title = lookup_string(
            candidate,
            &[
                "recommended_issue_title",
                "intent_title",
                "title",
                "intent_statement",
            ],
        )
        .unwrap_or_else(|| "Enable unmet intent".to_string());
        let symptom_title = lookup_string(candidate, &["symptom_title", "problem_statement"])
            .unwrap_or_else(|| "Observed symptom".to_string());
        let intent_title = lookup_string(candidate, &["intent_title", "recommended_issue_title"])
            .unwrap_or_else(|| issue_title.clone());
        let intent = lookup_string(candidate, &["intent_statement", "sample_intent"])
            .unwrap_or_else(|| intent_title.clone());
        let recommendation = lookup_string(candidate, &["recommendation"])
            .unwrap_or_else(|| format!("Add direct support for {intent_title}."));
        let volume = lookup_u64(
            candidate,
            &["failure_count", "workaround_count", "abandonment_count", "total_count"],
        )
        .unwrap_or(1);
        let success_rate = lookup_f64(candidate, &["success_rate"]).unwrap_or(0.0);
        let trend = if lookup_u64(candidate, &["abandonment_count"]).unwrap_or(0) > 0 {
            "growing"
        } else {
            "stable"
        };
        let kind = lookup_string(candidate, &["suggested_kind"]).unwrap_or_else(|| {
            if lookup_u64(candidate, &["workaround_count"]).unwrap_or(0) > 0 {
                "workaround".to_string()
            } else {
                "missing_capability".to_string()
            }
        });
        let dedupe_key = lookup_string(candidate, &["intent_key"]).unwrap_or_else(|| {
            normalize_key(&format!("intent:{intent_title}:{issue_title}"))
        });
        if existing_keys.contains(&normalize_key(&issue_title))
            || existing_keys.contains(&normalize_key(&intent_title))
            || existing_keys.contains(&dedupe_key)
        {
            continue;
        }
        existing_keys.insert(normalize_key(&issue_title));
        existing_keys.insert(normalize_key(&intent_title));
        existing_keys.insert(dedupe_key.clone());

        findings.push(json!({
            "kind": kind,
            "symptom_title": symptom_title,
            "intent_title": intent_title.clone(),
            "recommended_issue_title": issue_title.clone(),
            "title": issue_title,
            "intent": intent,
            "recommendation": recommendation,
            "priority_score": lookup_f64(candidate, &["priority_score"]).unwrap_or((0.50_f64 + (volume as f64 / 25.0)).min(0.9)),
            "volume": volume,
            "success_rate": success_rate,
            "trend": trend,
            "requires_spec_change": lookup_string(candidate, &["suggested_kind"]).unwrap_or_default() != "governance_gap",
            "problem_statement": lookup_string(candidate, &["problem_statement"])
                .unwrap_or_else(|| format!("{intent_title} is not directly supported today.")),
            "root_cause": format!("Recent trajectory evidence for '{}' clusters around '{}'.", intent_title, symptom_title),
            "spec_diff": recommendation,
            "acceptance_criteria": [
                format!("Users or agents can complete '{}' directly.", intent_title),
                "Observed failure/workaround patterns drop after the change."
            ],
            "dedupe_key": dedupe_key,
            "evidence": candidate.clone(),
        }));
    }

    for unmet in legacy_unmet_intents.iter().take(2) {
        let entity_type = lookup_string(unmet, &["entity_type"]).unwrap_or_else(|| "UnknownEntity".to_string());
        let action = lookup_string(unmet, &["action"]).unwrap_or_else(|| "UnknownAction".to_string());
        let error_pattern = lookup_string(unmet, &["error_pattern"]).unwrap_or_else(|| "UnknownError".to_string());
        let failure_count = lookup_u64(unmet, &["failure_count", "count"]).unwrap_or(1);
        let recommendation = lookup_string(unmet, &["recommendation"])
            .unwrap_or_else(|| format!("Add or repair {entity_type}.{action} handling."));
        let intent = lookup_string(unmet, &["sample_intent"])
            .unwrap_or_else(|| format!("Complete {action} on {entity_type}"));
        let intent_title = format!("Enable {}", humanize_issue_focus(&intent));
        let title = intent_title.clone();
        let dedupe_key = normalize_key(&format!("unmet:{entity_type}:{action}:{error_pattern}"));
        if existing_keys.contains(&normalize_key(&title)) || existing_keys.contains(&dedupe_key) {
            continue;
        }
        existing_keys.insert(normalize_key(&title));
        existing_keys.insert(dedupe_key.clone());

        let priority = (0.55_f64 + (failure_count as f64 / 20.0)).min(0.95);
        findings.push(json!({
            "kind": "missing_capability",
            "symptom_title": format!("{action} hits {error_pattern} on {entity_type}"),
            "intent_title": intent_title,
            "recommended_issue_title": title.clone(),
            "title": title,
            "intent": intent,
            "recommendation": recommendation,
            "priority_score": priority,
            "volume": failure_count,
            "success_rate": 0.0,
            "trend": "growing",
            "requires_spec_change": true,
            "problem_statement": format!("Users are trying to {action} on {entity_type}, but the capability is currently blocked by {error_pattern}."),
            "root_cause": format!("The current spec and implementation do not cover the requested {entity_type} workflow."),
            "spec_diff": format!("Add or extend {entity_type} support so agents can execute {action} without {error_pattern}."),
            "acceptance_criteria": [
                format!("Agents can execute {action} on {entity_type} without the current {error_pattern} failure."),
                "Observe metrics show the unmet-intent failure count drops after deployment."
            ],
            "dedupe_key": dedupe_key,
            "evidence": unmet.clone(),
        }));
    }

    for suggestion in policy_suggestions.iter().take(2) {
        let description = lookup_string(suggestion, &["description"])
            .unwrap_or_else(|| "Relax an over-restrictive policy path".to_string());
        let denial_count = lookup_u64(suggestion, &["denial_count", "count"]).unwrap_or(1);
        let title = if description.is_empty() {
            "Resolve repeated policy denials".to_string()
        } else {
            description.clone()
        };
        let dedupe_key = normalize_key(&format!("policy:{title}"));
        if existing_keys.contains(&normalize_key(&title)) || existing_keys.contains(&dedupe_key) {
            continue;
        }
        existing_keys.insert(normalize_key(&title));
        existing_keys.insert(dedupe_key.clone());

        findings.push(json!({
            "kind": "governance_gap",
            "symptom_title": title.clone(),
            "intent_title": "Enable direct issue workflow progression for worker agents",
            "recommended_issue_title": "Enable worker agents to move issues into todo",
            "title": "Enable worker agents to move issues into todo",
            "intent": "Complete the blocked workflow without repeated Cedar denials.",
            "recommendation": description,
            "priority_score": (0.45_f64 + (denial_count as f64 / 25.0)).min(0.85),
            "volume": denial_count,
            "success_rate": 0.0,
            "trend": "stable",
            "requires_spec_change": false,
            "problem_statement": "The intended issue-workflow outcome is blocked by repeated policy denials on the same transition.",
            "root_cause": "Authorization rules are narrower than actual usage patterns.",
            "spec_diff": "Adjust Cedar policy or app capabilities to align authorized behavior with real demand.",
            "acceptance_criteria": [
                "The repeated denial pattern is no longer observed for the intended workflow.",
                "Any widened policy remains scoped to the minimum required principals and resources."
            ],
            "dedupe_key": dedupe_key,
            "evidence": suggestion.clone(),
        }));
    }

    if findings.is_empty() {
        for feature in feature_requests.iter().take(1) {
            let description = lookup_string(feature, &["description"])
                .unwrap_or_else(|| "Address a repeated feature request".to_string());
            let frequency = lookup_u64(feature, &["frequency", "count"]).unwrap_or(1);
            let title = format!("Enable {}", humanize_issue_focus(&description));
            let dedupe_key = normalize_key(&format!("feature:{description}"));
            if existing_keys.contains(&normalize_key(&title)) || existing_keys.contains(&dedupe_key) {
                continue;
            }
            findings.push(json!({
                "kind": "workaround",
                "symptom_title": format!("Feature requests keep accumulating for {description}"),
                "intent_title": title.clone(),
                "recommended_issue_title": title.clone(),
                "title": title,
                "intent": description,
                "recommendation": description,
                "priority_score": (0.40_f64 + (frequency as f64 / 25.0)).min(0.8),
                "volume": frequency,
                "success_rate": 0.2,
                "trend": "stable",
                "requires_spec_change": false,
                "problem_statement": "Users are repeatedly asking for the same outcome outside the supported path.",
                "root_cause": "The feature is not part of the current product surface.",
                "spec_diff": "Review whether the capability should graduate into the main spec.",
                "acceptance_criteria": [
                    "The requested capability is either planned explicitly or closed with a documented rationale.",
                    "Duplicate feature requests no longer accumulate without a disposition."
                ],
                "dedupe_key": dedupe_key,
                "evidence": feature.clone(),
            }));
        }
    }

    if findings.is_empty() {
        for agent in agents.iter().take(1) {
            let agent_id = lookup_string(agent, &["agent_id", "id"]).unwrap_or_else(|| "unknown-agent".to_string());
            let total_actions = lookup_u64(agent, &["total_actions"]).unwrap_or(0);
            let success_rate = lookup_f64(agent, &["success_rate"]).unwrap_or(0.0);
            if total_actions == 0 {
                continue;
            }
            let title = format!("Reduce workflow friction for {agent_id}");
            let dedupe_key = normalize_key(&format!("friction:{agent_id}"));
            if existing_keys.contains(&normalize_key(&title)) || existing_keys.contains(&dedupe_key) {
                continue;
            }
            findings.push(json!({
                "kind": "friction",
                "symptom_title": format!("{agent_id} needs too many steps to complete common work"),
                "intent_title": title.clone(),
                "recommended_issue_title": title.clone(),
                "title": title,
                "intent": format!("Let {agent_id} complete common tasks with fewer steps."),
                "recommendation": "Review the top repeated workflow and collapse the multi-step sequence into a higher-level capability.",
                "priority_score": 0.35,
                "volume": total_actions,
                "success_rate": success_rate,
                "trend": "stable",
                "requires_spec_change": false,
                "problem_statement": "A high-volume workflow still requires too many manual steps.",
                "root_cause": "The current API surface is low-level relative to real usage patterns.",
                "spec_diff": "Consider a composed action that captures the common workflow directly.",
                "acceptance_criteria": [
                    "The workflow requires fewer state transitions than before.",
                    "Agent success rate stays stable or improves after the simplification."
                ],
                "dedupe_key": dedupe_key,
                "evidence": agent.clone(),
            }));
        }
    }

    let tenant = lookup_string(signal_summary, &["tenant"]).unwrap_or_else(|| "unknown-tenant".to_string());
    let summary = format!(
        "Mock evolution analysis for tenant {tenant}: {} intent candidates, {} workaround patterns, {} abandonment patterns, {} policy suggestions, {} feature requests, {} agent summaries, {} findings emitted.",
        intent_candidates.len(),
        workaround_patterns.len(),
        abandonment_patterns.len(),
        policy_suggestions.len(),
        feature_requests.len(),
        agents.len(),
        findings.len()
    );

    json!({
        "summary": summary,
        "findings": findings,
    })
}

fn collect_existing_dedupe_keys(signal_summary: &Value) -> std::collections::BTreeSet<String> {
    let mut keys = std::collections::BTreeSet::new();

    if let Some(issues) = signal_summary.get("issues").and_then(Value::as_array) {
        for issue in issues {
            if let Some(title) = lookup_string(issue, &["Title", "title", "name"]) {
                keys.insert(normalize_key(&title));
            }
            if let Some(dedupe_key) = lookup_string(issue, &["DedupeKey", "dedupe_key"]) {
                keys.insert(normalize_key(&dedupe_key));
            }
        }
    }

    if let Some(records) = signal_summary.get("recent_records").and_then(Value::as_array) {
        for record in records {
            if let Some(title) = lookup_string(record, &["title", "description", "problem_statement"]) {
                keys.insert(normalize_key(&title));
            }
        }
    }

    keys
}

fn lookup_string(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        let Some(candidate) = value.get(*key) else {
            continue;
        };
        if let Some(text) = candidate.as_str() {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        } else if candidate.is_number() || candidate.is_boolean() {
            return Some(candidate.to_string());
        }
    }
    None
}

fn lookup_u64(value: &Value, keys: &[&str]) -> Option<u64> {
    for key in keys {
        let Some(candidate) = value.get(*key) else {
            continue;
        };
        if let Some(number) = candidate.as_u64() {
            return Some(number);
        }
        if let Some(number) = candidate.as_i64() {
            if number >= 0 {
                return Some(number as u64);
            }
        }
        if let Some(text) = candidate.as_str() {
            if let Ok(number) = text.trim().parse::<u64>() {
                return Some(number);
            }
        }
    }
    None
}

fn lookup_f64(value: &Value, keys: &[&str]) -> Option<f64> {
    for key in keys {
        let Some(candidate) = value.get(*key) else {
            continue;
        };
        if let Some(number) = candidate.as_f64() {
            return Some(number);
        }
        if let Some(text) = candidate.as_str() {
            if let Ok(number) = text.trim().parse::<f64>() {
                return Some(number);
            }
        }
    }
    None
}

fn normalize_key(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}

fn humanize_issue_focus(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "unmet intent".to_string();
    }
    trimmed
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            format!(
                "{}{}",
                first.to_ascii_lowercase(),
                chars.as_str().to_ascii_lowercase()
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn detect_anthropic_oauth_mode(api_key: &str, auth_mode: &str) -> bool {
    match auth_mode.trim().to_ascii_lowercase().as_str() {
        "oauth" => true,
        "api_key" => false,
        _ => api_key.starts_with("sk-ant-oat"),
    }
}

/// Call Anthropic Messages API.
fn call_anthropic(
    ctx: &Context,
    api_key: &str,
    api_url: &str,
    model: &str,
    system_prompt: &str,
    messages: &[Value],
    tools: &[Value],
    anthropic_auth_mode: &str,
) -> Result<LlmResponse, String> {
    // Detect OAuth token (sk-ant-oat-*) vs standard API key
    let is_oauth = detect_anthropic_oauth_mode(api_key, anthropic_auth_mode);

    // OAuth tokens enforce a fixed system prompt when tools are present.
    // Custom system instructions are prepended to the first user message instead.
    let (effective_system, effective_messages) = if is_oauth {
        let oauth_system = "You are Claude Code, Anthropic's official CLI for Claude.".to_string();
        let mut msgs = messages.to_vec();
        if !system_prompt.is_empty() {
            if let Some(first) = msgs.first_mut() {
                if let Some(content) = first.get("content").and_then(|v| v.as_str()) {
                    let combined = format!("[System instructions: {system_prompt}]\n\n{content}");
                    first["content"] = json!(combined);
                }
            }
        }
        (oauth_system, msgs)
    } else {
        (system_prompt.to_string(), messages.to_vec())
    };

    let mut body = json!({
        "model": model,
        "max_tokens": 4096,
        "messages": effective_messages,
    });

    if !effective_system.is_empty() {
        body["system"] = json!(effective_system);
    }

    if !tools.is_empty() {
        body["tools"] = json!(tools);
    }

    let body_str =
        serde_json::to_string(&body).map_err(|e| format!("JSON serialize error: {e}"))?;

    ctx.log(
        "info",
        &format!(
            "llm_caller: calling Anthropic API, model={model}, oauth={is_oauth}, messages={}, url={api_url}",
            messages.len(),
        ),
    );

    // Build auth headers — OAuth tokens use Bearer + beta header
    let headers = if is_oauth {
        vec![
            ("authorization".to_string(), format!("Bearer {api_key}")),
            ("anthropic-version".to_string(), "2023-06-01".to_string()),
            (
                "anthropic-beta".to_string(),
                "oauth-2025-04-20,computer-use-2025-01-24".to_string(),
            ),
            ("content-type".to_string(), "application/json".to_string()),
            ("user-agent".to_string(), "claude-cli/2.1.75".to_string()),
            ("x-app".to_string(), "cli".to_string()),
        ]
    } else {
        vec![
            ("x-api-key".to_string(), api_key.to_string()),
            ("anthropic-version".to_string(), "2023-06-01".to_string()),
            ("content-type".to_string(), "application/json".to_string()),
        ]
    };

    // Retry on transient API errors (500, 529, and 400 with vague "Error" message)
    let mut last_err = String::new();
    let mut resp = None;
    for attempt in 0..5 {
        if attempt > 0 {
            ctx.log(
                "warn",
                &format!(
                    "llm_caller: retrying (attempt {}/5), last error: {last_err}",
                    attempt + 1
                ),
            );
        }
        match ctx.http_call("POST", api_url, &headers, &body_str) {
            Ok(r) if r.status == 200 => {
                resp = Some(r);
                break;
            }
            Ok(r) if r.status == 500 || r.status == 529 => {
                last_err = format!("HTTP {}: {}", r.status, &r.body[..r.body.len().min(200)]);
                continue;
            }
            Ok(r) if r.status == 400 && r.body.contains("\"message\":\"Error\"") => {
                // Transient 400 with vague error message — retry
                last_err = format!("HTTP 400 (transient): {}", &r.body[..r.body.len().min(200)]);
                continue;
            }
            Ok(r) => {
                return Err(format!(
                    "Anthropic API returned {}: {}",
                    r.status,
                    &r.body[..r.body.len().min(500)]
                ));
            }
            Err(e) => {
                last_err = e;
                continue;
            }
        }
    }
    let resp = resp.ok_or_else(|| format!("Anthropic API failed after 5 attempts: {last_err}"))?;

    let parsed: Value = serde_json::from_str(&resp.body)
        .map_err(|e| format!("failed to parse LLM response: {e}"))?;

    let stop_reason = parsed
        .get("stop_reason")
        .and_then(|v| v.as_str())
        .unwrap_or("end_turn")
        .to_string();

    let content = parsed.get("content").cloned().unwrap_or(json!([]));

    // Extract token usage from Anthropic response
    let usage = parsed.get("usage").cloned().unwrap_or(json!({}));
    let input_tokens = usage
        .get("input_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let output_tokens = usage
        .get("output_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    ctx.log(
        "info",
        &format!("llm_caller: usage: input={input_tokens}, output={output_tokens}"),
    );

    Ok(LlmResponse {
        content,
        stop_reason,
        input_tokens,
        output_tokens,
    })
}

/// Call OpenRouter Chat Completions API (OpenAI-compatible schema).
fn call_openrouter(
    ctx: &Context,
    api_key: &str,
    api_url: &str,
    model: &str,
    system_prompt: &str,
    messages: &[Value],
    tools: &[Value],
    site_url: &str,
    app_name: &str,
) -> Result<LlmResponse, String> {
    let mut or_messages = Vec::<Value>::new();
    if !system_prompt.is_empty() {
        or_messages.push(json!({
            "role": "system",
            "content": system_prompt,
        }));
    }
    or_messages.extend(convert_messages_to_openrouter(messages));

    let openai_tools = convert_tools_to_openrouter(tools);
    let mut body = json!({
        "model": model,
        "messages": or_messages,
        "max_tokens": 4096,
    });
    if !openai_tools.is_empty() {
        body["tools"] = json!(openai_tools);
        body["tool_choice"] = json!("auto");
    }

    let body_str =
        serde_json::to_string(&body).map_err(|e| format!("JSON serialize error: {e}"))?;

    let mut headers = vec![
        ("authorization".to_string(), format!("Bearer {api_key}")),
        ("content-type".to_string(), "application/json".to_string()),
    ];
    if !site_url.trim().is_empty() {
        headers.push(("HTTP-Referer".to_string(), site_url.trim().to_string()));
    }
    if !app_name.trim().is_empty() {
        headers.push(("X-Title".to_string(), app_name.trim().to_string()));
    }

    ctx.log(
        "info",
        &format!(
            "llm_caller: calling OpenRouter API, model={model}, messages={}, url={api_url}",
            messages.len(),
        ),
    );

    let mut last_err = String::new();
    let mut resp = None;
    for attempt in 0..5 {
        if attempt > 0 {
            ctx.log(
                "warn",
                &format!(
                    "llm_caller: openrouter retry (attempt {}/5), last error: {last_err}",
                    attempt + 1
                ),
            );
        }
        match ctx.http_call("POST", api_url, &headers, &body_str) {
            Ok(r) if r.status == 200 => {
                resp = Some(r);
                break;
            }
            Ok(r) if matches!(r.status, 429 | 500 | 502 | 503 | 504) => {
                last_err = format!("HTTP {}: {}", r.status, &r.body[..r.body.len().min(200)]);
                continue;
            }
            Ok(r) => {
                return Err(format!(
                    "OpenRouter API returned {}: {}",
                    r.status,
                    &r.body[..r.body.len().min(500)]
                ));
            }
            Err(e) => {
                last_err = e;
                continue;
            }
        }
    }
    let resp = resp.ok_or_else(|| format!("OpenRouter API failed after 5 attempts: {last_err}"))?;

    let parsed: Value = serde_json::from_str(&resp.body)
        .map_err(|e| format!("failed to parse OpenRouter response: {e}"))?;
    let choice = parsed
        .get("choices")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .cloned()
        .unwrap_or(json!({}));
    let message = choice.get("message").cloned().unwrap_or(json!({}));

    let mut content_blocks = Vec::<Value>::new();
    let text = extract_openrouter_text(&message);
    if !text.is_empty() {
        content_blocks.push(json!({
            "type": "text",
            "text": text,
        }));
    }

    let mut has_tool_calls = false;
    if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
        for (idx, tc) in tool_calls.iter().enumerate() {
            let fn_name = tc
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("unknown_tool");
            let call_id = tc
                .get("id")
                .and_then(Value::as_str)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("or_tool_{}", idx + 1));
            let args_str = tc
                .get("function")
                .and_then(|f| f.get("arguments"))
                .and_then(Value::as_str)
                .unwrap_or("{}");
            let input = serde_json::from_str::<Value>(args_str).unwrap_or(json!({}));

            content_blocks.push(json!({
                "type": "tool_use",
                "id": call_id,
                "name": fn_name,
                "input": input,
            }));
            has_tool_calls = true;
        }
    }

    let usage = parsed.get("usage").cloned().unwrap_or(json!({}));
    let input_tokens = usage
        .get("prompt_tokens")
        .and_then(|v| v.as_i64())
        .or_else(|| usage.get("input_tokens").and_then(|v| v.as_i64()))
        .unwrap_or(0);
    let output_tokens = usage
        .get("completion_tokens")
        .and_then(|v| v.as_i64())
        .or_else(|| usage.get("output_tokens").and_then(|v| v.as_i64()))
        .unwrap_or(0);

    let stop_reason = if has_tool_calls {
        "tool_use".to_string()
    } else {
        "end_turn".to_string()
    };

    Ok(LlmResponse {
        content: Value::Array(content_blocks),
        stop_reason,
        input_tokens,
        output_tokens,
    })
}

fn extract_openrouter_text(message: &Value) -> String {
    if let Some(text) = message.get("content").and_then(Value::as_str) {
        return text.to_string();
    }
    if let Some(arr) = message.get("content").and_then(Value::as_array) {
        let mut chunks = Vec::<String>::new();
        for item in arr {
            if let Some(text) = item.get("text").and_then(Value::as_str) {
                chunks.push(text.to_string());
            } else if let Some(text) = item.get("content").and_then(Value::as_str) {
                chunks.push(text.to_string());
            }
        }
        return chunks.join("\n");
    }
    String::new()
}

fn stringify_content(value: &Value) -> String {
    if let Some(s) = value.as_str() {
        s.to_string()
    } else {
        value.to_string()
    }
}

fn convert_messages_to_openrouter(messages: &[Value]) -> Vec<Value> {
    let mut out = Vec::<Value>::new();
    for msg in messages {
        let role = msg.get("role").and_then(Value::as_str).unwrap_or("user");
        let content = msg.get("content").cloned().unwrap_or(json!(""));

        match content {
            Value::String(text) => {
                out.push(json!({
                    "role": role,
                    "content": text,
                }));
            }
            Value::Array(blocks) => {
                if role == "assistant" {
                    let mut text_chunks = Vec::<String>::new();
                    let mut tool_calls = Vec::<Value>::new();
                    for (idx, block) in blocks.iter().enumerate() {
                        match block.get("type").and_then(Value::as_str).unwrap_or("") {
                            "text" => {
                                if let Some(t) = block.get("text").and_then(Value::as_str) {
                                    text_chunks.push(t.to_string());
                                }
                            }
                            "tool_use" => {
                                let id = block
                                    .get("id")
                                    .and_then(Value::as_str)
                                    .map(|s| s.to_string())
                                    .unwrap_or_else(|| format!("tool_{}", idx + 1));
                                let name = block
                                    .get("name")
                                    .and_then(Value::as_str)
                                    .unwrap_or("unknown_tool");
                                let input = block.get("input").cloned().unwrap_or(json!({}));
                                tool_calls.push(json!({
                                    "id": id,
                                    "type": "function",
                                    "function": {
                                        "name": name,
                                        "arguments": input.to_string(),
                                    }
                                }));
                            }
                            _ => {}
                        }
                    }

                    let mut assistant = json!({
                        "role": "assistant",
                        "content": text_chunks.join("\n"),
                    });
                    if !tool_calls.is_empty() {
                        assistant["tool_calls"] = json!(tool_calls);
                    }
                    out.push(assistant);
                } else if role == "user" {
                    let mut user_text = Vec::<String>::new();
                    for block in &blocks {
                        match block.get("type").and_then(Value::as_str).unwrap_or("") {
                            "tool_result" => {
                                let tool_call_id = block
                                    .get("tool_use_id")
                                    .and_then(Value::as_str)
                                    .unwrap_or("unknown_tool_call");
                                let content = stringify_content(
                                    block.get("content").unwrap_or(&Value::String(String::new())),
                                );
                                out.push(json!({
                                    "role": "tool",
                                    "tool_call_id": tool_call_id,
                                    "content": content,
                                }));
                            }
                            "text" => {
                                if let Some(t) = block.get("text").and_then(Value::as_str) {
                                    user_text.push(t.to_string());
                                }
                            }
                            _ => {}
                        }
                    }
                    if !user_text.is_empty() {
                        out.push(json!({
                            "role": "user",
                            "content": user_text.join("\n"),
                        }));
                    }
                } else {
                    out.push(json!({
                        "role": role,
                        "content": Value::Array(blocks),
                    }));
                }
            }
            other => {
                out.push(json!({
                    "role": role,
                    "content": other,
                }));
            }
        }
    }
    out
}

fn convert_tools_to_openrouter(tools: &[Value]) -> Vec<Value> {
    let mut out = Vec::<Value>::new();
    for tool in tools {
        let Some(name) = tool.get("name").and_then(Value::as_str) else {
            continue;
        };
        let description = tool
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("");
        let parameters = tool
            .get("input_schema")
            .cloned()
            .unwrap_or(json!({"type": "object", "properties": {}}));
        out.push(json!({
            "type": "function",
            "function": {
                "name": name,
                "description": description,
                "parameters": parameters,
            }
        }));
    }
    out
}

/// Build tool definitions for the LLM based on enabled tools.
fn build_tool_definitions(tools_enabled: &str, sandbox_url: &str, workdir: &str) -> Vec<Value> {
    let enabled: Vec<&str> = tools_enabled.split(',').map(str::trim).collect();
    let mut tools = Vec::new();

    if enabled.contains(&"read") {
        tools.push(json!({
            "name": "read",
            "description": format!("Read a file from the sandbox at {sandbox_url}. Working directory: {workdir}"),
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to read" }
                },
                "required": ["path"]
            }
        }));
    }

    if enabled.contains(&"write") {
        tools.push(json!({
            "name": "write",
            "description": format!("Write a file to the sandbox at {sandbox_url}. Working directory: {workdir}"),
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to write" },
                    "content": { "type": "string", "description": "File content" }
                },
                "required": ["path", "content"]
            }
        }));
    }

    if enabled.contains(&"edit") {
        tools.push(json!({
            "name": "edit",
            "description": format!("Edit a file in the sandbox at {sandbox_url} using search-and-replace. Working directory: {workdir}"),
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to edit" },
                    "old_string": { "type": "string", "description": "Text to find" },
                    "new_string": { "type": "string", "description": "Text to replace with" }
                },
                "required": ["path", "old_string", "new_string"]
            }
        }));
    }

    if enabled.contains(&"bash") {
        tools.push(json!({
            "name": "bash",
            "description": format!("Run a bash command in the sandbox at {sandbox_url}. Working directory: {workdir}"),
            "input_schema": {
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Bash command to execute" }
                },
                "required": ["command"]
            }
        }));
    }

    if enabled.contains(&"logfire_query") {
        tools.push(json!({
            "name": "logfire_query",
            "description": "Query Logfire observability data with either raw SQL or built-in intent-analysis patterns. Use this to inspect failure clusters, retries, alternate success paths, and abandonment evidence before producing final findings.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "sql": { "type": "string", "description": "Raw SQL query to run against Logfire records or metrics tables. Optional when query_kind is provided." },
                    "query_kind": { "type": "string", "description": "Optional built-in pattern query: recent_events, intent_failure_cluster, workflow_retries, alternate_success_paths, intent_abandonment" },
                    "service_name": { "type": "string", "description": "Optional service filter. Defaults to temper-platform." },
                    "environment": { "type": "string", "description": "Optional deployment_environment filter, e.g. local" },
                    "entity_type": { "type": "string", "description": "Optional entity/resource filter for built-in query kinds" },
                    "action": { "type": "string", "description": "Optional action filter for built-in query kinds" },
                    "intent_text": { "type": "string", "description": "Optional intent text filter for built-in query kinds" },
                    "agent_id": { "type": "string", "description": "Optional agent identifier filter for built-in query kinds" },
                    "lookback_minutes": { "type": "integer", "description": "Optional recency window for built-in query kinds. Defaults to 240." },
                    "min_timestamp": { "type": "string", "description": "Optional ISO timestamp lower bound" },
                    "max_timestamp": { "type": "string", "description": "Optional ISO timestamp upper bound" },
                    "limit": { "type": "integer", "description": "Optional row limit, clamped to 200" },
                    "row_oriented": { "type": "boolean", "description": "Return JSON rows instead of columns. Defaults to true." }
                },
                "required": []
            }
        }));
    }

    tools
}

/// Read conversation messages from TemperFS File entity via $value endpoint.
fn read_conversation_from_temperfs(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    file_id: &str,
    user_message: &str,
) -> Result<Vec<Value>, String> {
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = vec![
        ("x-tenant-id".to_string(), tenant.to_string()),
        ("x-temper-principal-kind".to_string(), "system".to_string()),
        ("accept".to_string(), "application/json".to_string()),
    ];

    match ctx.http_call("GET", &url, &headers, "") {
        Ok(resp) if resp.status == 200 => {
            let parsed: Value = serde_json::from_str(&resp.body).unwrap_or(json!({"messages": []}));
            let messages = parsed
                .get("messages")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            if messages.is_empty() {
                // First turn — initialize with user message
                Ok(vec![json!({ "role": "user", "content": user_message })])
            } else {
                Ok(messages)
            }
        }
        Ok(resp) if resp.status == 404 => {
            // File has no content yet — first turn
            ctx.log(
                "info",
                "llm_caller: TemperFS file has no content, initializing",
            );
            Ok(vec![json!({ "role": "user", "content": user_message })])
        }
        Ok(resp) => {
            ctx.log(
                "warn",
                &format!(
                    "llm_caller: TemperFS read failed (HTTP {}), falling back to inline",
                    resp.status
                ),
            );
            Ok(vec![json!({ "role": "user", "content": user_message })])
        }
        Err(e) => {
            ctx.log(
                "warn",
                &format!("llm_caller: TemperFS read error: {e}, falling back to inline"),
            );
            Ok(vec![json!({ "role": "user", "content": user_message })])
        }
    }
}

/// Write conversation messages to TemperFS File entity via $value endpoint.
fn write_conversation_to_temperfs(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    file_id: &str,
    conversation_json: &str,
) -> Result<(), String> {
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("x-tenant-id".to_string(), tenant.to_string()),
        ("x-temper-principal-kind".to_string(), "system".to_string()),
    ];

    // Wrap messages array in the TemperFS conversation format
    let body = format!("{{\"messages\":{conversation_json}}}");

    let resp = ctx.http_call("PUT", &url, &headers, &body)?;
    if resp.status >= 200 && resp.status < 300 {
        ctx.log(
            "info",
            &format!(
                "llm_caller: wrote conversation to TemperFS ({} bytes)",
                body.len()
            ),
        );
        Ok(())
    } else {
        Err(format!(
            "TemperFS $value write failed (HTTP {}): {}",
            resp.status,
            &resp.body[..resp.body.len().min(200)]
        ))
    }
}

fn resolve_temper_api_url(ctx: &Context, fields: &Value) -> String {
    fields
        .get("temper_api_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| match ctx.config.get("temper_api_url").map(String::as_str) {
            Some(value) if !value.trim().is_empty() && !value.contains("{secret:") => {
                Some(value.to_string())
            }
            _ => None,
        })
        .unwrap_or_else(|| "http://127.0.0.1:3000".to_string())
}
